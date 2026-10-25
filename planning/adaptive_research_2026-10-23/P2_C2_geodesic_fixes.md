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

## C9 — re-seed from the cleared area (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base 609ce75b
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`).
`rustfmt --edition 2024 --check` is clean on both Rust files. No shipped
path is changed (`path.rs`, `search.rs`, `spiral.rs`, `mod.rs`,
`material_grid.rs` are not touched; no visibility change was needed: the
module reads `material_count`, `cells`, `fringe_bits`, `SUB`,
`sub_offset`, `CELL_MATERIAL` and `CELL_FRINGE`, which are `pub(super)` or
`pub` in `material_grid.rs` and so visible inside `adaptive`).

Paths below are in `crates/rs_cam_core/src/adaptive/research_geodesic.rs`
unless named otherwise; line numbers are of the C9 tree.

### Keep-or-drop rule (set before any prediction)

Column `geodesic_curv_reseed` (`GeodesicOptions::C9_RESEED`) against
`geodesic_curv` (`C5_CURV`). KEEP when ALL three hold:

1. On the pocket, the cascade-class wrap refusals (the `clearing wrap`
   row of the cascade block, column "cascade") are at most half of
   `geodesic_curv`'s: at most 640 / 2 = 320.
2. Total entries do not rise above `geodesic_curv` on either shape: at
   most 73 on the star and at most 63 on the pocket.
3. The max emitted swept width is at most the ceiling 0.3626, except the
   known G-MOPLOAD mop step (a `residue mop` over-step counts as known).

Otherwise DROP, and the lead goes to idea 1b (split at the meeting line).
The operator's look decides after that.

### The design

**What re-seeds.** The producer walks its own plan through a replay model
while it plans (`ReplayModel`, :4047). The model is the replay half of the
cascade diagnostic, with the same rules (`CascadeWalk::step` :5128,
`CascadeWalk::link`, `cascade_readout` :5238): a step that holds
`measure_step` under the ceiling is stamped; a refused step is not, and the
cutter lifts; a lifted cutter re-starts at the next step that holds, with
the entry hole stamped (`LinkLoad::stamp_entry`, `path.rs:207`); a `Link`
holds when every step holds, else it is a re-entry at its end. The steps
are the replay's: `subdivide` at the pass step (`path.rs:405`), and the
model splits a wrap's join exactly as `append_path` (:3617) emits it (a
gap no longer than the reach is a cutting step, a longer one a `Link`).

**The trigger** (`ReplayModel::plan_wrap`, :4203): a WRAP step (not a join
step) that the model refuses while its cutter is down, once a wrap step of
the same field has cut stock. At the trigger:

1. the wrap ends before the refused step (the refused step is not
   planned, so the model cutter stays down there);
2. the field's remaining wraps are dropped: they were placed on stock the
   replay will not have cut;
3. a new field is solved from the cleared area (below), and the plan goes
   on from the cutter with that field's wraps (`ordered_wraps` :1972, the
   one-side meeting rule `meeting_trim` :1919 included).

This repeats (`reseed_segments`, :4366).

**Why this trigger, and not the meeting line only or the whole first
field.**

- A refused wrap step is the refusal that cascades. The replay stamps
  only accepted steps, so every later wrap of the field over that stretch
  meets the stock it left (D5: 640 of the 763 pocket wrap refusals are
  cascades). A refused JOIN is not a trigger: the replay re-enters at the
  wrap, and join refusals do not cascade (D5 star: 0 of 97).
- "Down" excludes the steps of a cutter that is hunting for a re-entry
  after a refused join; those are the J entries, not a cascade source.
  With them as triggers, every join refusal (97 on the star) would
  re-seed.
- The D5 render of `pocket / geodesic_curv` (`g1_2d_renders_v4.html`, the
  pale grey wraps under the brown residue contours and the magenta mop)
  shows that the refusals start where the fronts bend round the island,
  BEFORE the two branches meet. A re-seed at the meeting line comes after
  the cascade has begun, and a re-seed after the whole first field comes
  after every cascade. The refusal trigger is the earliest point at which
  a cascade can be stopped. It contains the meeting-line case: the far
  side's first refused wrap is a trigger.
- This is the simplest form that answers idea 1a: one rule, no new
  constant, and with no trigger it is the `geodesic_curv` plan (next
  paragraph).

**Consistency with the control.** With no trigger, `reseed_segments`
emits the same segments as the `Reseed::Off` path of the same options:
the same `ordered_wraps` call (start at the starter end, level 1), the
same `PassEntry` marker, every wrap through `append_path` with the same
reach. The model only reads. So `fields in use 1, 0 re-seeds` must give
`geodesic_curv`'s numbers exactly (73 / 63); a difference with 0 re-seeds
is a defect.

**The re-seeded field** (`GeodesicField::reseeded`, :796):

- `T = 0` on every reachable node where the cutter meets no stock in the
  model grid (`cleared_nodes`, :4269): no material cell centre within `R`,
  and no standing fringe sub-point within `R` (the points the swept width
  reads; the arithmetic of `clear_circle_inner`, `material_grid.rs:351`).
  These are the accepted passes' own stamps (the replay's material state),
  not the planned wraps.
- Why tool-centre nodes and not the stamped cells themselves. The field
  is in cutter-centre space: a wrap is one spacing from the cutter centres
  that cut. The edge of the cut stock lies `R` further out, so `T = 0` on
  the stamped cells would put the next wrap `R − s'` inside the cut.
  A node on a stamped step is cleared, because the stamp cleared every
  point within `R` of the step (`clear_capsule_inner`,
  `material_grid.rs:269`). The unit test checks this.
- Each cleared node is its own pivot (`t = 0`) and keeps its front branch
  from the field before. The branch table is handed over
  (`GeodesicField::branch_keys`), so a meeting line of the new field lies
  where the fronts from the two sides of the island meet again
  (`cell_on_shock` :1056 reads unrelated branches). The islands and the
  walls stay unreachable (the same `inside` mask and `Walls`).
- The solver is the source tracking of the first field. `track_sources`
  (:472) is split: its sweep is now `sweep_sources` (:511), which takes
  the start pivots and the branch table. `track_sources` builds the same
  start pivots (the seed centre, `t = −R`, branch 0) and an empty table,
  so the first field is unchanged, operation for operation.
- `T` at a node is the length of its pivot path to a cleared node. It is
  never less than the distance to the cleared area (each value is a pivot's
  value plus the distance to that pivot, down to a cleared node at 0). So
  a wrap lies at most its level from the cut. The over-read (the true
  cleared edge lies up to a cell diagonal past the last cleared node, plus
  the hand-off error of the 8-neighbour sweep) moves the wraps toward the
  cut: less stock, never more.

**The spacing of a re-seeded field** is the curvature law from `ρ_0 = R`
(`use_curvature_spacing` :961, `curvature_levels` :2472), the law of the
first field at its seed. The cleared area is treated as the starter lap.
The first new wrap lies `s'` from the cut: `ρ_1 − R` with
`ρ_1 = −(R − cD) + √((R − cD)² + (2R)² − R²)` (`law_next_radius` :2262),
1.437 mm at s 2, R 3, c 0.3626 (C5 table). The spacing reaches `s` at
`ρ* = s(2R − s)/(2(cD − s))`, 22.8 mm. Why not plain spacing: the first
plain wrap would lie where the refused wrap lay, on the same stock, and
the model would refuse it again. Known limit: the law assumes that the
cleared front has a radius of at least `R + T`. The end of a cut wrap (a
truncated wrap's own end) is a tighter point, and a pass round it can
still read over the ceiling. That refusal is a new trigger, not a
cascade.

**The stop rule** (derived; no count, no tolerance):

- a field that places every wrap without a trigger ends the plan
  (`stop Finished`);
- a re-seeded field with no level ends it (`stop NoLevel`): every
  reachable uncleared node lies within the law's first spacing `s'(ρ_1)`
  of the cleared area, so no wrap is left, and the residue passes take
  the rest (this is the coverage condition in field form: the levels
  start at the first spacing the law allows);
- a trigger needs a wrap step of the same field that held AND cut stock
  (the material cell count fell). So the stock count falls strictly
  between two re-seeds, and the number of re-seeds is finite (at most the
  number of material cells). A field whose wraps cut nothing before a
  refusal is placed whole, as under `Reseed::Off`, and the replay handles
  it.

### Readout

- `FieldReport::reseed` (`ReseedReport`, :3513): per field in use, its
  levels, the wraps placed, the uncleared reachable tool-centre area at
  its solve (mm²), the stock its planned steps cut in the model (mm²,
  material cells × cell²: the area each re-seed covered), where the
  trigger was (level and point), and convergence; the re-seed count and
  the stop.
- The harness prints `re-seed piece 0: N re-seeds, M fields in use, stop
  …` and one `field k:` line per field, for the C9 column only (the other
  columns' `.txt` files are unchanged).
- Entries by producer (S/L/J/M/C/W/P), the cascade block per class
  (refused / cascade / intrinsic), and the max emitted swept width are the
  existing lines. The wraps the harness draws are the emitted (truncated)
  wraps.

### Unit test

`the_reseeded_field_grows_from_the_cleared_area` (:6729), on
`island_pocket` (60 × 40 mm, island of radius 6 at (25, 20)), seed on the
island's axis at (8, 20). The grid is cut by the disc of every node with
`T ≤ |c − seed| − R` (the front at the island's centre line: part of the
way round, the far side standing). It asserts:

- every stamped node reads cleared (`cleared_nodes`);
- the re-seeded field holds `T = 0` exactly on the cleared nodes and
  `T > 0` on every other node, and it solves nodes on the far side
  (x > 25 + 6 + R);
- under the law its level 1 is `ρ_1 − R` from `ρ_0 = R` to 1e-9 (the law
  restarts at the cut);
- every point of level `k` lies within `L_k·s + √2·cell` of a cleared node
  (a contour point lies in a cell with a corner whose `T` is at most `L_k`;
  that `T` is a path length to a cleared node; the corner is at most a
  cell diagonal away), with points on the far side present.

### Predictions (NOT results; the run decides)

- **Re-seeds.** Star: about 3 to 10 (D5: 10 intrinsic wrap refusals).
  Pocket: about 5 to 20 (D5: 123 intrinsic wrap refusals, but after a
  re-seed the law's tight first spacings should remove most of them).
- **Pocket cascades.** Cascade-class wrap refusals 640 → under 100: after
  each trigger the stock the refusal left is planned again from the cut.
  Rule (1) predicted to hold.
- **Entries.** Each re-seed restarts the law, which costs about one extra
  level over the remaining area (C5: "one extra level out to ρ*"), and
  each extra level adds arcs, so joins. Star 70 to 85 (rule (2) at risk);
  pocket 50 to 70 (M and P fall where the cascade went; J may rise).
- **Max emitted swept width.** As `geodesic_curv` (0.3581 star; pocket
  0.4677 is the mop, G-MOPLOAD); the mop may cut less on the pocket.
- **Runtime.** Each re-seed is one source-tracking solve and one
  `ordered_wraps` call; the harness may take tens of seconds longer.

### Risks for the verifier

- About 700 new lines that no compiler has seen.
- The model is a model (C5 risk note): the shipped replay's walk
  (`emit_capped_walk_with`) has more cases. Read the cascade block of the
  C9 column (the diagnostic replays the emitted raw plan) as the outcome.
- The truncated wrap is emitted in the model's subdivision (collinear
  points at the pass step). A re-subdivision of a piece exactly one pass
  step long can split it in two by rounding; the plan is the same moves.

### Files changed by C9

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: `Reseed`,
  `GeodesicOptions::reseed` (Off in every existing preset),
  `GeodesicOptions::C9_RESEED`; `track_sources` split into
  `sweep_sources`; `GeodesicField::branch_keys`,
  `GeodesicField::reseeded`; `FieldReport::reseed`, `ReseedReport`,
  `ReseedField`, `ReseedStop`; `ReplayModel`, `cleared_nodes`,
  `reseed_segments` and the hook in `geodesic_segments` (:3702); the
  module doc section; the unit test.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the
  `geodesic_curv_reseed` column (default set) and the `re-seed` lines.
- This note.

### Commands for the verifier (C9)

Through `scripts/cargo_lane.sh`, each with a `timeout`, from the worktree:

```text
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

Read in this order:

1. **Controls (exact).** `geodesic_a` star 92 / pocket 88; `geodesic_curv`
   star 73 = S1 J58 M0 P14 (0.3581, raw 145/2545), pocket 63 = S1 J34 M12
   C6 P10 (0.4677, raw 260/2952); agent 25 / 31; contour_spiral 101 / 91.
   A difference is a defect of the `track_sources` split or the new
   option field: stop and report it.
2. `geodesic_curv_reseed`, per shape: the `re-seed piece 0` line and its
   `field k` lines (re-seeds, the area each covered, the stop); if it reads
   0 re-seeds, its numbers must equal `geodesic_curv`'s.
3. The `entries by producer` line (S/L/J/M/C/W/P).
4. The cascade block: `clearing wrap` refused / cascade / intrinsic on the
   pocket (rule (1): cascade ≤ 320), and per class on both shapes.
5. The max emitted swept width and its over-steps (rule (3)).
6. Then apply the keep-or-drop rule above.

## C11 — split at the meeting line (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base f576e51e
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`,
C9 committed). `rustfmt --edition 2024 --check` is clean on both Rust
files. No shipped path is changed (`path.rs`, `search.rs`, `spiral.rs`,
`mod.rs`, `material_grid.rs` are not touched) and no visibility changes:
the module reads `ToolCentreRegion::pieces` (`pub(crate)`) and
`Polygon2::holes` (`pub`).

Paths below are in `crates/rs_cam_core/src/adaptive/research_geodesic.rs`
unless named otherwise; line numbers are of the C11 tree.

### Keep-or-drop rule (set before any prediction)

Column `geodesic_curv_split` (`GeodesicOptions::C11_SPLIT`, :1809)
against `geodesic_curv` (`C5_CURV`). KEEP when ALL three hold:

1. On the pocket, the cascade-class wrap refusals (the `clearing wrap`
   row of the cascade block, column "cascade") are at most half of
   `geodesic_curv`'s: at most 640 / 2 = 320.
2. Total entries do not rise above `geodesic_curv`: at most 63 on the
   pocket and at most 73 on the star.
3. The max emitted swept width is at most the ceiling 0.3626, except the
   known G-MOPLOAD mop step (a `residue mop` over-step counts as known).

Otherwise DROP; the lead then takes the far side to the operator.

### The design

Operator idea 1b: split the region at the meeting line and clear the far
side ("the spaghetti zone") as its own pass. One new option value,
`Reseed::Zone` (:1721), in the preset `C11_SPLIT` = `C5_CURV` with
`reseed: Reseed::Zone`. The hook is in `geodesic_segments` (:3826): when
the zone is not empty, `split_segments` (:4858) plans the piece; when it is
empty (no island) the plan is the `Reseed::Off` plan, line for line (only
the report gets a `split` record).

**(1) The zone** (`far_side_zone`, :4696). The reachable, solved nodes
whose source-tracked front branch, or an ancestor of it, is a bend round a
HOLE ring of the tool-centre region (`Branches::bend` :406 keys a branch by
(parent, ring, side); `Walls::ring_near` :438 names the ring; the hole
rings are found in the same order, exterior then holes per piece). On a
part with one island and a convex outer wall this is exactly the set of
nodes the island hides from the seed: the shadow of the island.

Why this definition and not the other two of the brief:

- It is the brief's option 3 at no tolerance. Where a path does not bend,
  the node sees the seed centre `c`, its pivot is `c`, and the law's
  radius `ρ = R + T` is the front's own radius exactly (the law,
  `curvature_levels` :2539, is derived for that circle). Where the path
  bends at a node `b`, `T = |b − c| − R + |x − b|`, so
  `(R + T) − |x − b| = |b − c|` (more bends add to it). That is the
  distance from the seed to a bend node, never 0: the two radii disagree at
  EVERY shadow node, and the smallest disagreement is the seed's distance
  to the island. So no tolerance enters, and option 3 gives this set.
- It is the brief's option 2 ("beyond the meeting line, pivot a bend node
  behind the island") without the word "beyond": the meeting line lies
  inside the shadow (both branches that meet went round the island), and
  the part of the shadow before the meeting line is where the refusals
  start (C9 §"Why this trigger": the D5 render shows the refusals start
  where the fronts bend round the island, before they meet; D9's first
  trigger, (27.50, 35.31), lies at the island's upper tangent point
  (28.7, 36.4), computed below).
- Option 1 ("the nodes of the second branch") is not a property of the
  field: both branches reach the meeting line at the same `T`, by the
  definition of the line. "Second" is an order of cutting.

**Closed off.** The zone is bounded by the island, the walls and the two
shadow lines, the rays from the seed that touch the island. A ray from the
seed is a gradient line of `T` in the first region (`T = |x − c| − R`), so
the first region's wraps (circles about the seed) cross the border at a
right angle, as they end at a wall. The last step of a wrap at the border
meets only its band: the wrap below ends on the same ray, `s'` closer to
the seed, and its end disc (radius `R > s'`) covers the stock on the inner
side, so the cap beyond the border reads the band width `s'`.

Pocket geometry (Python check of the fixture `demo_pocket.svg`: rect 5..75
× 5..55 rx 8, island r 10 at (40, 30); tool-centre: 8..72 × 8..52, hole r
13; seed (18.5, 18.5) from D5). Seed to island centre 24.38 mm; the shadow
lines leave the seed at −4.08° and 60.36°; tangent points (39.0, 17.0)
and (28.7, 36.4). Prototype on a 0.5 mm lattice (the planner cell at R 3):
zone 3836 nodes, 959 mm², of 2312 mm² reachable (session scratchpad
`c11/zone.py`; its region test is my reading of the SVG, not the planner's
`ToolCentreRegion`).

**(2) The first region** (`split_segments` step 1). The first field with
the zone left out of its reachable nodes (`GeodesicField::with_inside`,
:908: the values, branches, pivots and levels unchanged). A level set is
built over cells with four reachable corners (`cell_reachable`), so every
wrap ends at the last cell before the border and none enters the zone. The
rest is the `geodesic_curv` plan: `ordered_wraps` (:2039), the one-side
meeting rule (it has nothing to do here: every first-region node is branch
0), `append_path` (:3727). In this region the law is exact.

**(3) The cut.** The plan so far (the starter pocket, the first region,
the open run) is walked through C9's replay model (`ReplayModel` :4189,
`walk`), and `cleared_nodes` (:4411) reads the cleared area from the
model's grid: the zone starts from the stock the replay will have cut, not
from the planned wraps.

**(4) The zone's seed: T = 0 on the cut (chosen), not its own entry.**

- An own entry at the zone's EDT maximum costs one plunge and a starter
  pocket (+1 entry at least). Its front is a circle that the island, the
  walls and BOTH shadow lines cut, so each level is up to four arcs, each
  with a join. And the planner has no EDT of a sub-region: the shipped EDT
  (`compute_boundary_distances`) measures the distance to the walls, not
  to the cut; the zone's EDT maximum would need a new transform.
- `T = 0` on the cut: no plunge. The zone's border with the cut is the
  two shadow lines, straight; the cut's edge along each is the row of the
  first region's wrap end discs, a scallop of `R − √(R² − (s/2)²)` = 0.17
  mm at s 2, R 3. The front grown from it is straight: each level is one
  arc per side, from the island or the meeting line to a wall.
  Prototype: the deepest zone node lies 31.97 mm from the upper line and
  32.23 mm from the lower one (split at the wedge bisector), so 16 and 17
  levels under the law from `ρ_0 = R` (15 and 16 at plain spacing).
- So `T = 0` on the cut has fewer entries: no plunge, and about one arc
  per level per side. It is C9's mechanism ONCE: one solve, no trigger.

The zone field (`GeodesicField::grown_from`, :819; `reseeded` :810 is now
a call of it with the field's own mask and branches, the same operations,
so C9 is unchanged): `T = 0` and a pivot at every cleared node, solved by
`sweep_sources` (:525) over the zone nodes and the cleared nodes only
(`zone_inside`), the islands and walls unreachable as before.

**The branches of the zone field** (`zone_labels`, :4736). With every
cleared node in branch 0, the two fronts that grow from the two shadow
lines would be one branch and meet in a kink that the one-side rule cannot
see (`cell_on_shock` :1108 needs two unrelated branches). So each cleared
node takes the branch of the zone node nearest it in lattice steps
(breadth first over the 8 neighbours from every zone node at once): the
cleared nodes along the upper line take the branch that went over the
island, those along the lower line the other. The two are children of
branch 0 and unrelated, so the zone field's fronts meet on a meeting line
and the one-side rule (C2) applies inside the zone. The branch table of
the first field is handed over, so the ids keep their parents.

**Spacing in the zone.** The curvature law from `ρ_0 = R`
(`use_curvature_spacing` :1013), C9's rule: the cut is treated as the
starter lap. On the straight border plain spacing would be exact (the law's
`ρ → ∞` limit is `s`); the law is never wider than plain (capped at `s`)
and costs about one level per side (above), and it protects the places
where the zone's front bends round the island's far side (a bend node
there makes a tight circle again).

**(5) The link.** The zone's wraps are ordered from the cutter's position
at the end of the first region (`ordered_wraps(&zone_field, region, cur,
…)`), and the first one is entered by `append_path`: a cutting step within
the reach, else one keep-down `Link`, which the replay admits under the
pass load or turns into a re-entry. One `PassEntry` marker for the whole
plan, as in every column.

**(6) The rest** is the shared cleanup of every column: the replay
(`step_within_pass_load`), the residue contours, the wall pass and the
mop.

**The D9 readout defect, fixed.** `reseed_segments` now records each
emitted wrap as (field, closed, level) and builds its `JoinReport` with
`seam_joins` (:4646), the rule of the one-field plan (closed to closed
one level up in the same field: closed seam; else closed to arc, or from
an arc; the last wrap has no join). A truncated wrap counts as open. With
0 re-seeds it gives the `geodesic_curv` counts (the star's `joins piece 0`
line must now read as `star_geodesic_curv.txt`). This changes that line
only; no move.

### Readout (the `.txt` of the new column)

- `split piece 0:` fields in use (1 or 2), zone nodes and mm², how it was
  found (the text "the nodes whose source-tracked path to the seed bends
  round an island"), the first field's branches and meeting-line cells in
  the zone, the first region's levels and wraps;
- `  zone field:` the seed (`T = 0` on N cleared nodes, mm²), levels,
  wraps, branches and meeting-line cells in the zone, zone nodes not
  reached, converged.
- Entries by producer (S/L/J/M/C/W/P), the cascade block and the max
  emitted swept width are the existing lines; the `joins piece` line now
  counts both fields.

### Unit test

`the_far_side_zone_lies_behind_the_island` (:7364), on `island_pocket`,
seed (8, 20) on the island's axis (C9's test position):

- the zone lies behind the island: every node the island hides from the
  seed (`!contains_segment(seed, node)`) is within one cell diagonal of a
  zone node, and every zone node within one cell diagonal of a hidden node
  (the bend sits on a lattice node); the first field's meeting line has
  cells in the zone;
- the first region's wraps never enter it: for every point of every
  `ordered_wraps` wrap of the restricted field, the nearest lattice node is
  a first-region node (a contour point lies on an edge of a kept cell, and
  the node nearest a point on an edge is one of its ends);
- the two regions cover the reachable area: each solved node is in
  exactly one; with the grid cut by the disc of every first-region node,
  every first-region node reads cleared, the zone field grown from the
  cleared nodes solves every zone node, it has at least two branches in
  the zone, and its two fronts meet there (meeting-line cells > 0).

### Predictions (NOT results; the run decides)

- **Star:** the star has no hole, so the zone is empty by construction and
  the plan is `geodesic_curv`'s: 73 = S1 J58 M0 P14, 0.3581, raw 145/2545.
  Its `.txt` must equal `star_geodesic_curv.txt` except the title and one
  `split piece 0: 1 fields in use; zone 0 nodes …` line. A difference is a
  defect of the hook.
- **Pocket, the zone:** about 3800 nodes, about 950 mm² (prototype 959),
  2 front branches, fields in use 2, zone levels 16 to 18, zone wraps 30 to
  45.
- **Pocket, the first region:** 32 levels (the first field's); wraps about
  35 to 45 (estimate: 4 closed loops, about 30 arcs between the walls, the
  island's near side and the two lines, about 9 arcs in the strip under
  the lower line).
- **Pocket, cascades:** `clearing wrap` cascade 640 → 20 to 150 (rule (1)
  predicted to hold): the law is exact in the first region, and the zone's
  front is straight where it meets the cut.
- **Pocket, entries:** 45 to 70. Each arc still costs about one join where
  the next arc is entered radially (the G1 join); the split cuts the lower
  arcs of `geodesic_curv` (bottom wall round to the meeting line) at the
  lower line, which adds arcs, while the far side loses its M and P work.
  **Rule (2) is at risk.**
- **Max emitted swept width:** at most 0.3626 apart from the mop (rule (3)
  predicted to hold; the replay holds every clearing step).

### Risks for the verifier

- About 400 new lines that no compiler has seen.
- The zone test asserts the shadow both ways to one cell diagonal. A
  hidden node next to a shadow line far from the island could take a bend
  at a neighbour whose nearest ring is the outer wall (a branch keyed to
  the exterior ring, so not in the zone). I expect the shorter path round
  the tangent point to win there, but if the test fails on that
  assertion, that is the cause; it is a lattice artefact of the border, not
  of the plan.
- `zone_labels` labels by lattice steps (8-neighbour breadth first), not by
  distance; only the cleared nodes beside the border matter, and there
  the nearest zone node is across the line.
- The model is a model (C5 risk note); read the cascade block of the
  column, which replays the emitted raw plan.

### Files changed by C11

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: `Reseed::Zone`,
  `GeodesicOptions::C11_SPLIT`, `FieldReport::split`, `SplitReport`;
  `GeodesicField::with_inside`, `GeodesicField::grown_from` (`reseeded`
  calls it); `seam_joins` and the join counts of `reseed_segments` (the D9
  defect); `far_side_zone`, `zone_labels`, `branches_in`,
  `meeting_cells_in`, `split_segments` and the hook in
  `geodesic_segments`; the module doc section; the unit test.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the
  `geodesic_curv_split` column (default set) and the `split` lines.
- This note.

### Commands for the verifier (C11)

Through `scripts/cargo_lane.sh`, each with a `timeout`, from the worktree:

```text
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

Read in this order:

1. **Controls (exact).** `geodesic_a` star 92 / pocket 88; `geodesic_curv`
   star 73 = S1 J58 M0 P14 (0.3581, raw 145/2545), pocket 63 = S1 J34 M12
   C6 P10 (0.4677, raw 260/2952); `geodesic_curv_reseed` 73 / 74 (its
   `.txt` differs from D9's only in the `joins piece 0` line); agent 25 /
   31; contour_spiral 101 / 91. A difference is a defect of the
   `grown_from` split or the new option value: stop and report it.
2. `geodesic_curv_split`, star: must equal `geodesic_curv` (zone 0 nodes).
3. Pocket: the `split piece 0` and `zone field` lines (zone area, fields 2,
   zone nodes not reached 0).
4. The `entries by producer` line (S/L/J/M/C/W/P).
5. The cascade block: `clearing wrap` refused / cascade / intrinsic on the
   pocket (rule (1): cascade ≤ 320).
6. The max emitted swept width and its over-steps (rule (3)).
7. Then apply the keep-or-drop rule above.

## C14 — traditional arc linking: one-way and zigzag (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base ea468814
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`).
`rustfmt --edition 2024 --check` is clean on the three Rust files. C12
edits `research_geodesic.rs` at the same time (its section "The 3D research
arm (C12)"); my hunks do not touch it.

Paths: `R` = `crates/rs_cam_core/src/adaptive/research_geodesic.rs`,
`P` = `crates/rs_cam_core/src/adaptive/path.rs`, `H` =
`crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`. Line numbers
are of the C14 tree.

### The operator ruling (2026-10-24), as the spec

- Two research columns. One-way: every arc in one direction, a return
  through what was just cut. Zigzag: each arc in the opposite direction to
  the one before.
- The short step-over "turn" at each arc end may exceed the steady ceiling
  c·D as an explicit, COUNTED exemption: its length is at most one stepover
  (or the derived minimum the geometry needs), its feed is reduced by its
  measured engagement, and each turn is reported with its real engagement.
- The SHIPPED rule is unchanged.

### Keep-or-drop rule (set before any prediction)

KEEP a column (`geodesic_curv_oneway`, `geodesic_curv_zigzag`) when ALL
three hold:

1. Star entries are at most half of `geodesic_curv`'s: at most 36.
2. No step that is not a turn exceeds the ceiling 0.3626 (the known
   G-MOPLOAD mop step excepted). Read: `steps on no turn (emitted)` line,
   and the `over:` lines of the emitted plan.
3. Every turn has length at most its bound and carries its reduced feed.
   Read: the `arcs piece` line (`turn length .. to ..` ≤ the bound), and
   `turns with the reduced feed N, without 0`.

Otherwise DROP. After the rule, the operator's look decides.

### The design

**(1) The option.** `ArcLink { Nearest, OneWay, Zigzag }` (R:1738), field
`GeodesicOptions::arcs`; presets `C14_ONEWAY` (R:1853) and `C14_ZIGZAG`
(R:1859), each `C5_CURV` with the one field changed. `G1` and `C2` set
`arcs: ArcLink::Nearest`; every other preset inherits it, so every existing
column plans as before. The links apply with `JoinRule::Seam` and
`Reseed::Off`. **Not on the split base:** `split_segments` plans its own
two fields with its own `append_path` loop, and the replay model of C9/C11
(`ReplayModel`) would need the turn exemption too. That is not a one-line
preset, so it is not built.

**(2) The order: a lobe to its end first.** Under both links
`ordered_wraps` takes a child of the wrap just cut first (R:2138), the rule
the ramp join already had. Without it the nearest-wrap order jumps between
arms, and the next arc is not at the same wall.

**(3) One-way: climb.** The code's convention: a climb cut with a clockwise
spindle has the wall (the stock) on the RIGHT of the travel
(`compute/operation_configs.rs` doc of the bore `climb` flag;
`ops/bore.rs` module doc and `climb_turns_counter_clockwise_p6_6`). The
stock side of a level-set arc is where `T` grows. `climb_oriented`
(R:2088) reads `T` one cell right and one cell left of each segment's
middle and turns the arc so that `T` grows to the right (R:2190). Closed
wraps keep their lap direction.

**(4) The turn (R:4252, in the Seam loop of `geodesic_segments`).** For two
consecutive wraps that are both open arcs:

- zigzag: the turn is the step from the cutter (the end of arc k) to the
  near end of arc k+1 (the start `ordered_wraps` gave it);
- one-way: the turn is the step from the START of arc k to the start of
  arc k+1 (both climb, so both starts are at the same wall). Before it, the
  cutter goes back along arc k by one keep-down `Link` per vertex (the C8
  pattern): the band just cut, zero load, judged by the shipped
  `feed_link_checked`. A refused link becomes a re-entry, as everywhere.
- The turn is planned when its length is at most the bound (below) and the
  straight step lies inside the tool-centre region
  (`ToolCentreRegion::contains_segment`). Else the join stays as in
  `geodesic_curv` (a seam step the replay refuses, or a link) and is
  counted (`over_bound`, `outside`).

**(5) The bound (derived).** `turn_bound(s, cell) = s + (1 + √2)·cell`
(P:60): 3.207 mm at s 2, cell 0.5.

- Two consecutive arcs are `Δ ≤ s` apart in `T`.
- Each arc end carries the field error, at most one cell (the module's
  "Derived constants"; the disc unit test pins it).
- Each arc ends on the edge of the last cell with four reachable corners,
  at most one cell diagonal short of the wall.
- So on a wall the front meets at a right angle, the ends are at most
  `Δ + cell + √2·cell` apart. This is the producer's `reach`; the replay
  computes it again from `params.stepover` and the planner cell.
- **Oblique walls (C6).** Where the level sets meet the wall at an angle θ
  off the normal, the ends are `Δ / cos θ` apart. Past the bound the turn is
  not planned, and the count says how often (`over the bound`). I did not
  build a curved turn: on a straight or concave wall the straight chord
  between the two ends runs along the wall and lies inside the region. On a
  convex wall (round an island) the chord can leave the region; that is
  counted as `outside`. A turn path along a convex wall would need the
  boundary walk; a ruling decides whether that is worth it.

**(6) The replay exemption (the threading; shipped behaviour unchanged).**

- `LinkLoad` gains `turns: Vec<(P2, P2)>` and `turn_bound` (P:138), empty
  and 0 on every shipped path.
- `LinkLoad::admits_turn(a, b)` (P:217) is true only when (a) both ends of
  the step lie on a listed turn's segment (within 1e-6 mm, the walker's own
  same-point tolerance; the walk splits a turn by linear interpolation)
  and (b) that turn is no longer than the bound.
- `emit_capped_walk_with` (P:562):
  `holds = reading <= ceiling || load.admits_turn(a, b)`. With an empty list
  the `||` arm is false, so the decision is the old one.
- `apply_residue_mop_cleanup_with` and
  `apply_contour_parallel_residue_cleanup_with` keep their signatures and
  call new `_turns` variants (P:1744, P:1892) with `&[]`. In a `_turns`
  variant only the replay of the kept segments gets the turns
  (`LinkLoad::with_turns`, a copy). The residue contours, the wall pass and
  the mop get the plain `LinkLoad`, so they hold the ceiling with no
  exemption. With an empty list no copy is made.
- (c), the feed: an `AdaptiveSegment` carries no feed, so the replay cannot
  check it. The feed is written on the emitted toolpath by the research
  run (`apply_turn_feeds`, R:6318), from the measured engagement of the
  emitted plan. The readout then counts every emitted turn with no move
  that carries it (`without`, must be 0). This is (c) by construction plus
  a count, not a check inside the replay; I state it as a limit.

**(7) The feed relation and its source.** `turn_feed_factor(c, w, a) =
c / max(w, a)` when that is under 1 (R:6301); `w` is the swept width over
D, `a` the area per mm over D, the largest over the turn's steps.

- **Relation: hold the material removal rate of the steady cut.** The
  removal rate of a move is `a_p × (area per mm) × F`. On the steady side
  cut the area per mm equals the swept width (C7, Cavalieri), at most c·D.
  So `F' = F · c·D / (area per mm)`.
- **Source.** This is the shear term of the codebase's power model
  (`tool_load/power.rs`, `P = A·(Ks·MRR + F_edge·a_p·V_c·z·ψ/2π)`); the
  feed solver states "only the shear term moves with feed"
  (`dressup/feed_modulation.rs`). The edge term does not move with feed, so
  no feed holds it. It rises on a turn, and the record says so.
- **The swept width is the second reading.** A head-on cap reads its span
  in swept width and its removal in area. The lattice under-reads a thin
  cap in area (C7). The larger of the two decides.
- **The lateral force is held too.** The canonical force model
  (`feeds/force.rs`: `F = a_p·(Ks·f_z·sin θ_peak + F_edge)`,
  `θ_peak = min(ψ, π/2)`, `cos ψ = 1 − 2·a_e/D`) needs only
  `F' = F·sin θ_peak(c)/sin θ_peak(w)`. For every `w > c` the removal
  factor `c/w` is the smaller one: below `w = 1/2`,
  `(c/w)² < c(1−c)/(w(1−w))` because `c/w < 1 < (1−c)/(1−w)`; above it,
  `c/w < 2c ≤ 2√(c(1−c)) = sin θ_peak(c)`. The unit test pins both.
- **Chip thinning is not used.** The operator ruled it out
  (G-CHIPTHIN-HALFFIX, `dressup/feed_modulation.rs`).
- **No constant enters.** Worked value (exact geometry, Python): a head-on
  turn of 2 mm at R 3 in two 1 mm steps meets caps of 1 mm and 2 mm. The
  chords are 4.47 mm and 5.66 mm (0.745 D and 0.943 D). The area per mm of
  the second step is 5.15 mm²/mm (0.859 D). So the factor is
  0.3626 / 0.943 = 0.385. The force alone would need 0.9615 (at
  `w > 1/2`, `sin θ_peak(w) = 1`).

**(8) The readout.**

- `StepPart::Turn` (R:5297): `PartIndex::on_turn` (R:5384) classes a step
  as a turn by the replay's own test, before every other part. The raw
  plan's classes gain `clearing turn`. The emitted readout gets an index of
  the turns alone (`PartIndex::of_turns`), so its other steps stay
  `clearing`, as before (R:6662).
- `PassLoadReadout` gains `turn_steps` (each turn step with its swept width
  and area per mm), `turn_over`, `non_turn_max` and `non_turn_over`
  (R:5522). A turn step over the ceiling is not in `steps_over` or `over`
  (the replay admits it); it is counted in `turn_over` (R:5661).
- The cascade model admits a turn as the replay does (R:5946).
- `Summary2d::turns` (`TurnSummary`, R:6244) holds one record per emitted
  turn: ends, length, steps, steps over, max swept, max area, feed factor
  and feed (mm/min), and the fed and unfed counts.
- `FieldReport::arcs` (`ArcReport`, R:3638): the arc-to-arc joins, turns
  planned, over the bound, outside the region, the turn length range, and
  the return links (count, mm).
- Harness (H): the two columns after `geodesic_curv_split`.
  - The `arcs piece` line.
  - `steps on no turn (emitted)`: rule 2.
  - `clearing turns (emitted)`: rule 3.
  - The quartiles of turn swept width, area per mm, feed factor and
    length: the distribution.
  - One `turn:` line per turn.
  - These lines print only for a column with the links enabled, so every
    control's `.txt` is unchanged.

### Unit tests

- `P` `link_load_tests` (they need the private walker):
  - `a_step_over_the_ceiling_off_every_turn_is_refused` (P:3485): a
    corridor pass that ends with a 2 mm head-on step up the wall. The step
    is refused with no turn listed, with a turn listed elsewhere, and with
    a listed turn of 4 mm (over the 3.21 mm bound).
  - `a_planned_turn_within_the_bound_is_admitted` (P:3509): the same step
    on a listed 2 mm turn is admitted, with no re-entry.
- `R` `tests`:
  - `the_turn_feed_holds_the_steady_removal_and_the_lateral_force`
    (R:7741): for 20 engagements from c to 1 D, `factor·w = c` and
    `factor·sin θ_peak(w) ≤ sin θ_peak(c)` (`feeds::force::immersion_angle`);
    the larger reading decides; a turn under the ceiling keeps the full
    feed.
  - `a_clearing_turn_is_bounded_and_fed_down_and_no_other_step_passes`
    (R:7778), both links on the island pocket: turns are planned; every
    planned turn ≤ the bound; every emitted turn carries `F·factor` on its
    moves (`unfed` 0) and `factor·max(w, a) = c`; the emitted `clearing`
    and `clearing link` classes have 0 steps over the ceiling.

### Predictions (NOT results; the run decides)

- **Controls byte-identical** (the new field defaults to `Nearest`; empty
  turn lists change no decision; the new summary lines print only for the
  new columns). `geodesic_a` 92/88, `geodesic_curv` 73/63, `reseed` 73/74,
  `split` 73/84, agent 25/31, contour_spiral 101/91.
- **Turn engagement:** a head-on step of about Δ at the wall. Swept width
  0.75 to 0.94 D per step, area per mm 0.52 to 0.86 D. So the max per turn
  is about 0.94, and the feed factor is about 0.38 to 0.45 (shorter `Δ`
  under the curvature law near the seed gives a smaller cap).
- **Star, zigzag:** J falls from 58. The arc-to-arc seam steps inside a
  lobe become turns. The closed-to-closed seams (16) and the closed-to-arc
  joins stay refused, and arcs at other walls or in other arms stay links.
  Expected entries 35 to 55. **Rule 1 (≤ 36) is at risk;** the closed seams
  alone hold J at about 16 or more.
- **Star, one-way:** entries about the same as zigzag or a few more. A
  return link that the shipped check refuses near a wall becomes a re-entry
  (M). Keep-down link length grows by about the cut length of the lobe
  arcs (+200 to +600 mm). Cut length is about the same.
- **Pocket:** the long arcs between the rectangle's walls give turns on
  both sides. J falls from 34; entries 40 to 60. Some joins round the
  island count as `outside` (convex wall).
- **Rule 2:** holds by construction for the clearing classes (the replay
  admits nothing else); the mop over-step (G-MOPLOAD) can remain.
- **Rule 3:** holds by construction (the bound is the planning test and the
  replay test). `without` must be 0. A non-zero `without` means a turn's
  moves were merged or blended in the toolpath (no corner radius is set,
  so I do not expect it).

### Risks for the verifier

- About 600 new lines that no compiler has seen.
- `apply_turn_feeds` matches moves to turns by geometry (start and target
  on the turn segment within 1e-6 mm). With a corner radius > 0 the blend
  arcs at a turn's corners keep the full feed. The 2D default and the
  harness use 0.
- The return links: one `Link` per arc vertex (one per cell), so the
  `keep-down links` count grows by hundreds. Read the mm, not the count.
- `climb_oriented` skips samples off the reachable cells. An arc that lies
  entirely within one cell of a wall could get no sample and keep its
  order. That is not expected on the development shapes.
- The C14 unit test `a_clearing_turn_is_bounded…` asserts that turns are
  planned on the island pocket (a prediction pinned as a test). If it is
  red on that assertion alone, report the `ArcReport` it prints.

### Files changed by C14

- `crates/rs_cam_core/src/adaptive/path.rs`: `turn_bound`, the
  `LinkLoad::turns` / `turn_bound` fields, `with_turns`, `admits_turn`, the
  `holds` line of `emit_capped_walk_with`, the two `_turns` cleanup
  variants (the `_with` functions call them with `&[]`), the test
  `scene()` literal, and two unit tests.
- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: `ArcLink`,
  `GeodesicOptions::arcs`, `C14_ONEWAY`, `C14_ZIGZAG`, `climb_oriented`,
  the order and orientation in `ordered_wraps`, `ArcReport` and
  `FieldReport::arcs`, `Produced::turns`, the turn block in
  `geodesic_segments`, `StepPart::Turn`, the `PartIndex` turns, the
  readout fields (`TurnStep`, `turn_over`, `non_turn_*`), the cascade
  admission, `TurnSummary`, `TurnRecord`, `turn_feed_factor`,
  `apply_turn_feeds`, `cleaned(.., turns)`, `run_2d_with`, and two unit
  tests.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the two
  columns, the `arcs piece` line, `turn_lines`.
- This note.

### Commands for the verifier (C14)

Through `scripts/cargo_lane.sh`, each with a `timeout`, from the worktree:

```text
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test a_clearing_cut_holds_the_pass_load_g_adaptpassload
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test a_link_feeds_only_within_the_pass_load_g_adaptlinkload
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive_property_harness
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

`path.rs` changed, so the three adaptive pass-load sentries are added to
C11's list. `--lib adaptive` runs the two new `link_load_tests`.

Read in this order:

1. **Controls (exact):** `geodesic_a` 92/88; `geodesic_curv` 73 = S1 J58 M0
   P14 / 63 = S1 J34 M12 C6 P10; `geodesic_curv_reseed` 73/74;
   `geodesic_curv_split` 73/84; agent 25/31; contour_spiral 101/91. Compare
   each control `.txt` byte for byte with D10's. A difference is a defect of
   the threading: stop and report it.
2. For each new column, shape by shape:
   - `entries by producer` (S/L/J/M/C/W/P) and the entries;
   - `arcs piece 0` (turns, over the bound, outside, length range, return
     links);
   - `clearing turns (emitted)` (turns, steps over, fed / without), and the
     quartile lines;
   - `steps on no turn (emitted)` (rule 2);
   - `keep-down links` (count, mm) and `cut length`;
   - the cascade block.
3. Apply the keep-or-drop rule above.

## C16 — local-level spacing (a) under the one-way turns (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base c3d38bf4
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`).
`rustfmt --edition 2024 --check` is clean on the three Rust files. C18
works in `adaptive3d/` and `tests/adaptive3d_geodesic_arm_c12.rs` at the
same time; my hunks in `research_geodesic.rs` do not touch the C12 block.

Paths: `R` = `crates/rs_cam_core/src/adaptive/research_geodesic.rs`,
`S` = `crates/rs_cam_core/src/finish/scallop_isofield.rs`, `H` =
`crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`. Line numbers
are of the C16 tree. Prototype scripts (Python, scratchpad, not in the
repo): an SVG reader of the D11 renders and an emulation of
`iso_contour_where` + `chain_segments`.

### Keep-or-drop rule (set before any prediction; the lead's rule)

KEEP `geodesic_local_oneway` when ALL four hold, else DROP:

1. Its entries are strictly below `geodesic_curv_oneway` on both shapes:
   < 39 on the star, < 54 on the pocket.
2. No non-turn step is over the ceiling 0.3626 (the G-MOPLOAD mop step
   excepted). Read: `steps on no turn (emitted)` and the `over:` lines.
3. Every turn is within its bound and fed: `turns with the reduced feed
   N, without 0`, and the `arcs piece` turn length ≤ the bound.
4. On the pocket: no residue-contour entry (C = 0), and wrap cascades ≤ 11
   (the cascade line `clearing wrap: ... cascade N`).

### The design

**(1) `Spacing::LocalLevel`** (R:2008; C15 §5, idea (a)).
`GeodesicField::use_level_spacing` (R:1085) keeps the source-tracked `T`
from the seed and writes ONE value `T_k` per level, so the levels stay
parallel level sets of one field. This differs from C6's
`CurvatureLocal`, which solves a pass count `U` (non-parallel levels).

- The rule per level: from `T_{k-1}`, the step `ΔT` is the largest for
  which every point `p` of the trial level set `T_{k-1} + ΔT` lies within
  `min(s, s'(ρ(p)))` of the cut.
- The gap: `T` is a distance. A point of the trial level lies `ΔT·s`
  along its shortest path from the level before, so its straight
  distance to the cut is at most `ΔT·s`. The test
  `ΔT·s ≤ min_p min(s, s'(ρ(p)))` (`level_gap`, R:1132) is the rule read
  on the path length. It can space levels closer than needed behind a
  wall; it never under-reads the gap.
- `ρ(p)`: the circle through the level-set points at ± one pass step
  (`pass_step_len` = 3 cells = 1.5 mm) of arc length (`front_radius`,
  R:1191). Its sign comes from `T` one cell toward and one cell away from
  the centre: a centre on the cleared side (lower `T`) is a convex front
  and reads `ρ`; a concave or straight piece reads `s` (C15 "The level
  rule"). Where a sample is off the reachable cells the piece is taken
  as convex (the smaller spacing).
- `s'(ρ)` is read at the trial (outer) level, which is the root form of
  `law_next_radius` (`ρ' = ρ + s'(ρ')`). On a disc about the seed
  `ρ = R + T`, so the rule gives the C5 levels (unit test 2 below).
- Arc ends: a vertex with less than one pass step of its piece on one
  side gives no reading, and a piece shorter than two pass steps gives
  none. The kink at a meeting line is not read: the `Split` rule leaves
  the shock cells out of the contour (`chains_at`, R:1354).
- The bisection: `ΔT = 1` when it holds. Else halve `[0, 1]` until the
  bracket is at most the op tolerance (0.1 mm, the precision of
  `radius_at`), and take the lower end (the side that holds). That is 5
  halvings at s 2 (bracket 0.0625 mm), so at most 6 contourings per
  level. No new constant. The test is not monotone in `ΔT` in general,
  so the bisection finds a value that holds within the tolerance of one
  that fails.
- The walk ends when the next level passes `max T` (the rule of
  `curvature_levels`). A guard ends it if no step above the first
  bracket holds (a radius under about 0.75 mm, which a three-point
  reading over 1.5 mm spans cannot return).
- The transition-join drift reads `LocalLevel` as `Curvature` (R:4622).
  The re-seed and split fields keep `curvature_levels` (these plans are
  not C16 columns).

**(2) `Chaining`** (R:2014), the arm fix (below). `Chaining::TailOnly` is
every column before C16. `Chaining::Whole` contours with
`iso_segments_where` (S:474, split out of `iso_contour_where` S:456; a
pure refactor, `iso_contour_where` is `chain_segments` of it) and chains
with `chain_whole` (R:1599). `chain_whole` grows each chain at its tail
and then at its head and keeps a chain of one segment. The field carries
its chaining; re-seeded and zone fields keep their parent's.

**(3) Presets** (R:2198 to 2216). `C16_LOCAL` = `C2` with `LocalLevel` and
`Whole` (column `geodesic_local`, plain seam). `C16_LOCAL_ONEWAY` = that
with `ArcLink::OneWay` (column `geodesic_local_oneway`). An attribution
column `geodesic_curv_oneway_whole` (`C16_ONEWAY_WHOLE` = `C14_ONEWAY`
with `Whole`) tells the chaining's effect from the spacing's. It is not
under the rule.

**(4) Readouts.** `FieldReport::level_values_mm`; `TurnRecord::fed`
(R:6722); `ArcReport::over_bound_turns` with `OverBoundTurn` (R:4037,
measure only). The harness writes them to a separate file
`<shape>_<column>.c16.txt` for every candidate (a) column (`c16_text`,
H:756). The `.txt` of every earlier column is unchanged.

### Task 2: the lower-left star arm (the arm to (5, 35)) — CAUSE FOUND, research-side fix built

**Cause.** The arm's wraps are not whole arcs. `chain_segments`
(`geometry/marching_squares.rs`) grows each chain at its tail only and
drops any chain of fewer than three points. `iso_contour_where` emits the
segments in row-major cell order (world `y` up, then `x` up). On an arc
that falls to the right (negative slope in world XY), the scan meets the
cells against the arc's direction. Where the arc is steep (one cell per
row), the chains come out two segments long, and the one segment between
two chains is a two-point chain: dropped. `join_fragments` then cannot
bridge the hole. Loops are not affected (a loop chains round to its
start). The fault is in how the research producer uses the shared
chainer, not in the field.

**Why that arm.** With the seed at (49.5, 50.5), the arcs of the arm to
(5, 35) run at slopes −1 to −2.5, so they fall to the right. Its mirror
twin (the arm to (95, 35)) has arcs that rise to the right and loses no
segment. The other arms are in between. The seed one cell off the mirror
axis (G1-3) is not the cause: the fault is in the sign of the slope.

**Evidence** (the D11 renders, `target/tmp/adaptive_geodesic_2d_g1/`):

- Wrap pieces in the arm: almost all are 3 to 7 points long and about
  1 mm long. Most of them are classed CLOSED, because the two ends of a
  two-segment piece are 0.5 mm apart (≤ √2·cell). An example: the piece
  (32.46, 49.00) (32.50, 48.67) (32.52, 48.50) is cut as a loop.
- The holes: the fragment ends of one level are one cell row apart. On
  the level `T = 7.0495` stepovers (level 8), the pieces end and start at
  (32.20, 42.50) / (32.44, 42.00) and at (32.70, 41.50) / (32.97, 41.00):
  one cell row is missing between each pair.
- Gaps between fragment ends (≤ 0.75 mm) per render: star
  `geodesic_curv` 20, `geodesic_a` 19, `geodesic_g1` 19 (18 of the 20 in
  this arm, 2 in the arm to (79, 91) at (62.79, 61.25) and (64.19,
  59.25), where the arcs also fall to the right). Pocket `geodesic_curv`
  19 (the far side at x 66 to 71, and x 37 to 46 by y 44 to 49).
- The emulation of `iso_contour_where` + `chain_segments` on the exact
  field `(|x − seed| − R)/s` at level 8 gives the same pieces to 0.01 mm.
  Example: (31.97, 43.00) (32.00, 42.94) (32.20, 42.50). On a strip of
  the arm it drops 80 of 160 segments over five levels. On the mirror
  strip it drops 6 (the end segments).
- Entries by location, star `geodesic_curv_oneway` (39 entries):
  clearing entries in the arm proper (y < 47) **11**. Its mirror twin has
  1, the other arms 1, 1 and 3. Five more lie on the seam ray y = 50.5
  to the left of the seed. These are the closed-wrap seam refusals that
  every column has. In `geodesic_curv` the arm reads 13 clearing entries
  against 11 in each other arm: the seam join already re-enters once per
  arc, so the defect shows mostly under the one-way links.
- The residue: the mop runs through the arm from (34.00, 54.50) to
  (21.13, 44.19) and at its tip (14.5, 38.5). That is the stock that the
  dropped segments and the half-cut "loops" leave.

**Fix** (`Chaining::Whole`, opt-in for the C16 columns, so the controls
are unchanged). Unit test 1 pins the drop and the fix.

### Task 3: the unfed turn (zigzag, `without 1` on each shape) — CAUSE FOUND

- **Which turn.** A zigzag "turn" that bridges one of the chaining gaps of
  task 2. A dropped segment splits ONE arc into two pieces, and the
  zigzag link joins them with a turn of one cell length along the arc
  itself. Star candidates: (36.41, 39.50) → (36.00, 40.00) and its
  mirror (62.59, 61.50) → (63.00, 61.00), both 0.643 mm, 1 step, factor
  1.0000. Pocket candidates: the short turns at the pocket's gap points,
  (69.56, 35.50) → (69.93, 35.00) 0.617 mm, (10.50, 50.57) → (10.23,
  50.50) 0.284 mm, (13.50, 8.31) → (13.13, 8.50) 0.419 mm, (9.98, 11.00)
  → (9.56, 11.50) 0.650 mm and (42.01, 49.00) → (42.41, 48.50) 0.642 mm.
  Each is a gap of the list in task 2.
- **Why it is unfed.** The turn is the missing contour segment, so the arc
  continues on its line. Exact values (emulation, level 8): the turn is A
  (36.409393, 39.5) → B (36.004510, 40.0). The arc goes on to C
  (36.0, 40.005845), 7.4 µm on. Node (36, 40) lies 0.004 mm off the
  level, so the contour crosses two edges of that node 7 µm apart. B
  lies **0.17 µm** from the chord A–C. That is under the emitter's
  `MIN_EMITTED_SEGMENT_MM` = 1 µm, so
  `simplify_path_keeping_reversals` (`path.rs`) can drop B. Then the
  emitted move is A → C, and C is not on the turn segment (7.4 µm past
  B), so `apply_turn_feeds` finds no move. The drop happens only when B
  is the inner point of the RDP leaf (B and C are 0.01 µm apart in their
  distance to the higher chord). So of the two mirror turns only one is
  unfed. This is D11's candidate (collinear-point merging in the
  emission), confirmed in its mechanism.
- **Fix.** Research-side: `Chaining::Whole` removes the gaps, so no turn
  bridges a piece of one arc. The C16 columns carry it. The controls keep
  the defect (byte parity). The new readout names the unfed turn
  (`unfed turn:` lines in `.c16.txt`), so the verifier can confirm which
  one.

### Task 4: slanted-wall turns — MEASURE ONLY (the bound is not changed)

- Readout: `.c16.txt`, `joins over the bound` and one `over:` line per
  join longer than `s + (1 + √2)·cell` = 3.207 mm. Each line gives the
  join, its length, `θ` and the slanted bound `s / cos θ + (1 + √2)·cell`.
- `θ`: the angle between the join and the normal of the arc it goes onto,
  read from that arc's chord over one pass step from its start. With
  `θ = 0` (arcs square to the wall), the slanted bound is the C14 bound.
- The values come from the run. `geodesic_curv_oneway` (4 star, 10 pocket
  joins) is the control column for the table. `geodesic_local_oneway` and
  `geodesic_curv_oneway_whole` print theirs too. The table to fill:

| Shape / column | Join (from → to) | Length mm | θ deg | Slanted bound mm | Fits |
|---|---|---|---|---|---|
| (from the `.c16.txt` of the run) | | | | | |

### Unit tests (C16, research module)

1. `the_whole_chaining_keeps_the_segment_the_tail_chaining_drops`
   (R:7623): three segments of one arc, listed in the order that loses
   one. `chain_segments` keeps 2 of 3; `chain_whole` gives one arc of 4
   points end to end. A square loop comes back as one closed loop of 4
   points.
2. `the_local_level_spacing_follows_the_law_on_a_disc` (R:7668): a
   tool-centre disc of radius 20, the seed at the centre. Every step is at
   most `s` and within `tolerance + cell` of `law_next_radius` from the
   same level (the bisection bracket plus the contour's placement on the
   lattice).

### Predictions (NOT results; the run decides)

- **Controls byte-identical.** All presets before C16 set
  `Chaining::TailOnly`, the field's default, and `S` is a pure split. So
  every control `.txt` is unchanged: `geodesic_a` 92/88, `geodesic_curv`
  73/63, `reseed` 73/74, `split` 73/84, `oneway` 39/54, `zigzag` 50/66,
  agent 25/31, contour_spiral 101/91. `summary.txt` differs only by the
  three new column blocks. The `.c16.txt` files are new.
- **Zigzag `.c16.txt`:** one `unfed turn:` line per shape. Star: one of
  the two 0.643 mm turns above. Pocket: one of the five short turns above.
- **Levels.** Star: about the C5 levels (19), each step within 0.1 mm
  under the C5 step (the seed sees the whole star). Pocket: more levels
  than `geodesic_curv`, with the far side's tighter radius setting the
  step of the whole level. C15 read 35 exact levels against 32 at the
  harness levels.
- **`geodesic_curv_oneway_whole`:** star entries 27 to 33 (the arm to
  (5, 35) falls from 11 clearing entries to about 1, as its twin). Pocket
  46 to 54.
- **`geodesic_local_oneway`:** star 27 to 33 (the spacing changes little
  on the star). Pocket 42 to 52: the far-side wraps at their own radius
  read under the ceiling (C15: interior 0.3574 against 0.4129 at the
  harness levels), so fewer wrap refusals. Risk: more levels means more
  arcs and joins. Each over-bound join is still a seam step.
- **`geodesic_local`:** star 68 to 73 (the seam join re-enters once per
  arc whatever the chaining). Pocket 55 to 65.
- **The rule (predicted):** (1) likely on the star, uncertain on the
  pocket. (2) holds by construction. (3) fed holds, because whole chains
  leave no gap turn. (4) C = 0 likely; wrap cascades ≤ 11 uncertain.
  Overall: uncertain, leaning KEEP.

### Risks for the verifier

- About 450 new lines that no compiler has seen.
- Run time: about 6 contourings per level and about 35 levels per field,
  each a full-lattice contour plus `chain_whole`. I expect seconds, not
  minutes, in release.
- `front_radius` treats a sample off the reachable cells as convex. Near
  a wall that can read a straight piece as a tight one only if the three
  points bend; a lone false reading sets the step of a whole level. If a
  level step in `.c16.txt` is far under its neighbours, look for it.
- `level_values_mm` is the first field only (the re-seed and split plans
  print their first field).

### Files changed by C16

- `crates/rs_cam_core/src/finish/scallop_isofield.rs`:
  `iso_segments_where` split out of `iso_contour_where` (no behaviour
  change; `iso_contour_where` was added by candidate (a) and has no other
  caller than `iso_contour`).
- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: module doc
  section and derived constants; `Spacing::LocalLevel`, `Chaining`,
  `GeodesicOptions::chaining` and three presets; `GeodesicField::chaining`,
  `chains_at`, `use_level_spacing`, `level_gap`, `front_radius`,
  `level_values_mm`; `chain_whole`, `point_at_length`; `FieldReport::
  level_values_mm`; `OverBoundTurn` and `ArcReport::over_bound_turns`;
  `TurnRecord::fed`; two unit tests.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the
  three columns, `c16_text` and the `.c16.txt` file, the module doc.
- This note.

### Commands for the verifier (C16)

Before the run, copy D11's output dir (for the byte compare). Then, through
`scripts/cargo_lane.sh`, each with a `timeout`, from the worktree:

```text
cp -r target/tmp/adaptive_geodesic_2d_g1 <scratch>/d11_renders
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --all-targets
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --lib scallop_isofield
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
for f in <scratch>/d11_renders/*.txt; do cmp "$f" target/tmp/adaptive_geodesic_2d_g1/"$(basename "$f")" || echo "DIFF $f"; done
```

`path.rs` and `search.rs` are not changed, so the pass-load sentries are
not needed. Only `summary.txt` may differ, by the three new column blocks.

Read in this order:

1. **Controls (byte parity):** every D11 `.txt` but `summary.txt` must be
   identical. A difference is a defect of the threading: stop and report.
2. For each new column, shape by shape: `entries by producer`
   (S/L/J/M/C/W/P); `arcs piece 0`; `clearing turns (emitted)` and
   `without`; `steps on no turn (emitted)`; the cascade block
   (`clearing wrap` cascades); the `.c16.txt` levels and steps.
3. The zigzag `.c16.txt` (`unfed turn:`) for task 3; the
   `geodesic_curv_oneway` `.c16.txt` (`over:` lines) for the task 4 table.
4. Entries by location in the star arm to (5, 35) (x < 40, y < 47) in
   `geodesic_curv_oneway_whole` against `geodesic_curv_oneway`.
5. Apply the keep-or-drop rule above to `geodesic_local_oneway`.

## C20 — degenerate turn and the 3D departure rule (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base c60ed442
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`).
`rustfmt --edition 2024 --check` is clean on the three Rust files. The C12
block of `research_geodesic.rs` is not touched.

Paths: `R` = `crates/rs_cam_core/src/adaptive/research_geodesic.rs`,
`H` = `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`,
`T` = `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs`,
`P` = `crates/rs_cam_core/src/adaptive/path.rs`. Line numbers are of the
C20 tree. The SVG reader is in the session scratchpad (`c20/near3.py`,
not in the repo); it calibrates the D12 renders on the island
(centre (40, 30), radius 10).

### Task 1: the degenerate unfed turn — CAUSE FOUND

**What the two turns are.** The D12 renders near each turn (0.012 mm
SVG precision):

| Column | Arc before the turn (`prev`) | Arc after it (`w`) |
|---|---|---|
| pocket `local_oneway` | 3 points: (27.528, 25.000) (27.864, 24.503) (27.995, 24.289), about 0.83 mm | 64 points from (27.502, 25.037) up and left |
| pocket `curv_oneway_whole` | 3 points: (27.025, 25.998) (27.436, 25.497) (27.502, 25.423) | 60 points from (27.001, 26.023) |

- `prev` and `w` are two pieces of ONE level contour. Each turn's two ends
  are the two crossings of one lattice cell: (27.5284, 25.0) on the bottom
  edge and (27.5, 25.0387) on the left edge of the cell [27.5, 28] ×
  [25, 25.5]; (27.0230, 26.0) and (27.0, 26.0260) of the cell [27, 27.5] ×
  [26, 26.5].
- That cell is not reachable. Its corner (28, 25.5) is 12.816 mm from the
  island centre, and (27.5, 26.5) is 12.981 mm: both inside the island's
  tool-centre disc (10 + R = 13 mm). So the contour loses that cell's
  segment, and the level falls into a long arc and a short tail at the
  island wall. `Chaining::Whole` cannot join them: the segment is not
  there.
- Climb order puts both starts at the gap. The one-way turn (start of
  `prev` to start of `w`) is the missing segment itself: 35 to 48 µm,
  along the level. So it engages nothing (swept 0; area 0.0833 = one
  cell / D, the lattice floor) and its feed factor is 1.
- It continues the line of `w`'s first segment: the angle between the turn
  (126.3°) and `w`'s first segment (about 127.9°, ± 0.6° from the SVG
  rounding) is about 1.6°.

**Why no move carries it (elimination on the code).**

- The readout reads one step on the turn in the cleaned segments (else
  `apply_turn_feeds` skips it and prints no record). In the segments the
  cut after the return links is `[S_prev, S_w, w1, ...]` (render span 916:
  (27.527, 25.004) (27.502, 25.037) (27.141, 25.497) ...), and the link
  before it ends at `S_prev`.
- `segments_to_toolpath` maps a `Link` to one move and a `Cut` to one move
  per point of `simplify_path_keeping_reversals(path, MIN_EMITTED_SEGMENT_MM)`
  after the first (corner radius 0 in the harness). The simplifier never
  drops the first point. Nothing else removes a point.
- So the turn has no move on it only if the simplifier dropped `S_w`: `S_w`
  lies within 1 µm of the chord from `S_prev` to a later point of `w`. The
  1.6° kink over 0.048 mm and 0.58 mm puts `S_w` about 1.2 µm off the chord
  `S_prev`–`w1` (at the edge of the SVG precision; a longer leaf chord on a
  curving `w` gives a smaller distance at `S_w`). This is the C16 task 3
  mechanism (the zigzag gap turn, measured there at 0.17 µm).
- The run measures it. The `.c16.txt` line of each turn with no move on it
  now names the emitted move that holds it and the distance of each turn
  end from that move (`HoldingMove`). A distance under 1 µm confirms the
  drop.

**The fix (the readout rule, derived).** A turn with no emitted move on it
is unfed only when it needs a reduced feed.

- `turn_feed_factor` (R:6807) returns 1 exactly when the measured
  engagement `max(w, a)` is at most the ceiling. The turn's feed is then
  `F · 1 = F`.
- Every clearing move is emitted at `F` (`segments_to_toolpath`), and
  `apply_turn_feeds` only lowers feeds (`f.min(feed)`). So every move that
  holds the turn carries at most `F`: the turn holds its feed by
  construction.
- So such a turn is counted in `TurnSummary::full_feed_unmatched` (R:6712),
  not in `unfed` (R:6871). No constant enters.
- Why not the other fix (attach the feed to the move that holds the turn):
  that move can be much longer than the turn (here it runs on along `w`),
  so the reduced feed would cover cut that does not need it. It would also
  change the control toolpaths and hide the real defect of the pocket
  zigzag control: its gap turn (42.0063, 49.0) → (42.4098, 48.5) needs
  factor 0.7158, and it stays `without 1` under this rule. The fix for that
  kind is the whole chaining (C16), not a feed on a merged move.

**The limit.** A turn that needs a reduced feed and whose vertex the
emitter drops stays at the full feed. The readout still counts it as
unfed, and now names the move that holds it.

**Code.**

- R: `TurnSummary::full_feed_unmatched`; `TurnRecord::holding`;
  `HoldingMove` (R:6750) and `holding_move` (R:6761); the count in
  `apply_turn_feeds` (R:6836, doc); preset `C20_LOCAL_ZIGZAG` (R:2219,
  `C16_LOCAL` with `ArcLink::Zigzag`).
- R unit test `a_turn_whose_vertex_the_emitter_drops_is_unfed_only_when_it_needs_a_cut_feed`
  (R:8356): a hand-made toolpath with the turn's end vertex dropped. At
  swept 0 and one cell of area: fed 0, unfed 0, full-feed 1, and the
  holding move has both turn ends on it (under 1 µm). At 2c: unfed 1. With
  the vertex kept at 2c: fed 1, the turn's move carries `F · factor` and
  the next move `F`.
- H: the column `geodesic_local_zigzag` after `geodesic_curv_oneway_whole`;
  in `c16_text`, the line `turns at the full feed (factor 1) with no move
  on them, not unfed: N` (only when N > 0), the line kind (`unfed turn` or
  `full-feed turn, no move on it`), and the holding move on each such line
  (`holding_text`).

**The zigzag gap turns (0.643 / 0.642 mm).** C16 task 3 found them to be
the chaining gaps (one dropped segment splits one arc in two). The whole
chaining keeps every segment of a reachable cell, so no zigzag turn
bridges two pieces of one arc. `geodesic_local_zigzag` (LocalLevel +
Whole + Zigzag) shows it in the run. A one-way or zigzag join across a
masked cell (the kind above) can still occur; under the new rule it is
not unfed when it engages nothing.

### Task 2: the C12 3D oracle's departure exemption

**The 2D walker's rule (P).**

- P:655 `let departure = plunged_at.take();` in `replay_kept_segments`:
  only the first cut after a straight plunge gets the departure. P:675: a
  `Link` clears it. The research readout does the same
  (`pass_load_readout`, `plunged_at.take()`).
- P:544 `let departing = departure.is_some_and(|d| .. < r)`: a step is
  exempt by where it starts.
- P:551 `if was_departing && !departing`: the exempt departure stays a cut
  of its own. So in the emitted plan the first cut after the plunge ends at
  the first step that starts at `R` or more.

**The test's rule (T:331 `departing_moves`).** From a vertical
`EntryPlunge`, each move with XY travel is exempt while it starts within
`R` of the plunge. The first move that starts at `R` or more ends the
exemption until the next plunge. A helix or ramp entry, or a rapid with XY
travel, also ends it; vertical moves do not change it. C18's
`plunge_origin` (every move within `R` until the next entry) is removed.

**The one difference (stated in the doc of `departing_moves`).** The
emitted 3D path carries no mark of the end of a 2D cut. When the
departure cut ends within `R` (a short cut) and a link (emitted as a cut,
`links_as_cuts`) or another cut follows within `R` with no entry, the 2D
readout judges that move and this test exempts it. That needs a cut
shorter than `R` from its plunge with no entry after it.

**The ignore.** The prediction below is 0 over in both modes, so the
`evidence:` reason with its open condition goes. The test is not made
plain `#[test]`: it runs 427 s (D12), and a 7-minute test in every
`--features research,test-support` run breaks the operator's 3-minute
rule. It is now `#[ignore = "heavy: ..."]` (T:641), the f152 kind for "a
real assertion that takes minutes". The lead can drop the attribute if
the operator wants it in the full gate.

### Predictions (NOT results; the run decides)

**2D harness.**

- Plans, SVG geometry and PNGs: byte-identical for every column (no
  planner change). New files: `<shape>_geodesic_local_zigzag.{txt,svg,png,c16.txt}`.
- Files that change (readout only), and only these:
  - `pocket_geodesic_local_oneway.txt` and `.svg` (caption):
    `without 1` → `without 0`. Its `.c16.txt`: `turns without the reduced
    feed: 0`, the new full-feed line `1`, and the line `full-feed turn, no
    move on it: (27.5284, 25.0000) to (27.5000, 25.0387) ... held by the
    emitted move (27.5284, 25.0000) to (x, y) ..., turn ends 0.000e0 and
    < 1e-3 mm off it`.
  - `pocket_geodesic_curv_oneway_whole.txt`, `.svg`, `.c16.txt`: the same
    for (27.0230, 26.0000) → (27.0000, 26.0260).
  - `star_geodesic_curv_zigzag.txt`, `.svg`, `.c16.txt`: its gap turn
    (36.4094, 39.5) → (36.0045, 40.0) has factor 1 (swept 0.3147, area
    0.3441, both under 0.3626), so `without 1` → `without 0`; its
    `.c16.txt` names the holding move (both ends under 1 µm; C16 measured
    B at 0.17 µm).
  - `pocket_geodesic_curv_zigzag.c16.txt`: the `unfed turn` line gains the
    holding move (both ends under 1 µm). Its `.txt` is unchanged
    (`without 1`: factor 0.7158).
  - `summary.txt`: the changes above and the two new blocks.
- Every other `.txt`, `.c16.txt` and `.svg`: byte-identical to D12.
- **`geodesic_local_oneway` under the D12 rule:** (1) 34 < 39, 41 < 54;
  (2) star: the residue mop step only (excepted), pocket 0; (3)
  `without 0` both shapes, every turn ≤ the bound; (4) pocket C 0,
  cascades 5. Predicted verdict: **KEEP**.
- **`geodesic_local_zigzag`:** `without 0` on both shapes; no turn of the
  0.64 mm gap kind. Entries: star 30 to 45, pocket 40 to 60 (low
  confidence). A full-feed line can appear at the island's masked cells.

**C12 test (`--ignored`).**

- Entry, vertical, waterline counts: unchanged per arm (those checks come
  first).
- `departing + perimeter band + judged` is unchanged per arm: control
  798 + 870 + 1295 = 2963; `geodesic_curv` 5770 + 1665 + 2484 = 9919;
  `geodesic_curv_split` 4488 + 2029 + 2902 = 9419.
- `departing` falls in both modes (the starter spiral's 45 moves within R
  stay exempt: they run on from the plunge; D12 starter probe). Judged
  rises by most of the difference.
- **0 over the exact bound in both modes** (medium confidence). The newly
  judged moves are later moves that pass within R of an old plunge
  hole. The 2D planner judged those steps too, on a grid with the hole
  cut, and a cut hole lowers their engagement. If a sample is over, it is
  a real 3D/2D difference to classify, not an exemption to widen.
- Test result: green.

### `geodesic_local_zigzag`: required column, the same rule (lead, 2026-10-24)

The operator mills both directions by default, so zigzag is judged on the
same base as one-way. The column and its `.c16.txt` are already built (H:
the column after `geodesic_curv_oneway_whole`; `c16_text` writes a
`.c16.txt` for every candidate (a) column). The pre-registered rule, as for
`geodesic_local_oneway`:

| Rule | Limit | Read from |
|---|---|---|
| 1. entries | star < 39, pocket < 54 | `entries (plunges)` |
| 2. no non-turn step over 0.3626, mop excepted | 0 | `steps on no turn (emitted)` and the `over:` lines |
| 3. every turn within its bound and fed | `without 0`; turn length ≤ 3.207 mm | `clearing turns (emitted)`; `arcs piece` |
| 4. pocket: no residue-contour entry, few wrap cascades | C = 0, `clearing wrap` cascades ≤ 11 | `entries by producer`; the cascade block |

Prediction (NOT a result): (3) holds (no chaining gap; a masked-cell
join reads full feed, not unfed). (1) is the risk: `geodesic_curv_zigzag`
read 50 / 66 against `geodesic_curv_oneway` 39 / 54, so zigzag pays more
entries than one-way on the same base. A zigzag turn starts where the
cutter stands; where the next arc's near end is past the bound, the join
is a seam step or a link, not a turn. Predicted entries: star 30 to 45,
pocket 40 to 60. Verdict: uncertain.

**Which column is the default if both pass** (a recommendation; the
operator decides). The criterion, read from the run in this order:

1. Plunges: the column with fewer entries on both shapes. A plunge costs
   a retract, a rapid and a plunge, and each is a new departure.
2. Keep-down length: one-way returns along each arc it has just cut
   (D12 `geodesic_local_oneway`: 2029 links, 975.3 mm on the pocket,
   1129 / 967.4 mm on the star; most of it is return links, `return
   links` in `arcs piece`). Zigzag has no return. That length is feed-rate
   travel with no cut, about two thirds of the pocket's cut length (1488
   mm).
3. Load at the turns: the turn feed-factor quartiles. A lower factor is a
   heavier turn, so the cutter runs slower over it. Both columns admit
   the same bound and both feed down by the same rule, so the turns'
   load is held either way. Only their time differs.

If the entries are within a few of each other, I recommend
`geodesic_local_zigzag` as the default. It saves about 1 m of keep-down
travel per shape, which matches the operator's both-ways default. One-way
keeps every arc a climb cut, at the cost of that return travel. If zigzag
has clearly more entries (as at C14: +11 star, +12 pocket), one-way is the
default, because a plunge costs more than its return travel.

### Files changed by C20

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: as above.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the
  column, the module doc, `c16_text`, `holding_text`.
- `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs`:
  `departing_moves` in place of `plunge_origin`, its use in `read`, the
  module doc, the ignore reason.
- This note.

### Commands for the verifier (C20)

Before the run, copy D12's render dir (for the byte compare). Then, from
the worktree, through the lane script, each with a `timeout`:

```text
cp -r target/tmp/adaptive_geodesic_2d_g1 <scratch>/d12_renders
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_geodesic_arm_c12
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test every_ignore_states_its_kind_f152
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
for f in <scratch>/d12_renders/*.txt <scratch>/d12_renders/*.svg <scratch>/d12_renders/*.png; do cmp -s "$f" target/tmp/adaptive_geodesic_2d_g1/"$(basename "$f")" || echo "DIFF $f"; done
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 3600 scripts/cargo_lane.sh clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
```

The C12 test without `--ignored` must report 0 run, 1 ignored. With
`--ignored` it is about 7 min (D12: 427 s): the operator's go-ahead is
needed on the operator's PC. `path.rs`, `search.rs` and `adaptive3d/` are
not changed, so the 2D pass-load sentries and the adaptive3d sentries are
not needed; byte parity is run because `research_geodesic.rs` is in the
research build of the 3D arm.

Read in this order:

1. The byte compare: only the files in the prediction list differ. Any
   other difference is a defect: stop and report.
2. The `.c16.txt` holding-move lines: both turn ends under 1e-3 mm
   confirms the emitter's collinear drop.
3. `geodesic_local_oneway` under the D12 rule.
4. `geodesic_local_zigzag`: the four-rule table above, then the default
   criterion (entries, keep-down mm, turn feed factors) against
   `geodesic_local_oneway`.
5. The C12 summary lines: the per-arm sums, `departing`, and the over
   counts.

## C23 — pocket far corner and island-side plunges (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base cdc9814e
(worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`).
`rustfmt --edition 2024 --check` is clean on the two Rust files. C22 edits
the C12 block of `research_geodesic.rs`, `adaptive3d/` and the C12 test at
the same time; my hunks do not touch them, and no signature that the C12
block calls changed (`geodesic_segments`, `cleaned`, `pass_load_readout`,
`PartIndex`, `TurnRecord`, `TurnSummary`).

Paths: `R` = `crates/rs_cam_core/src/adaptive/research_geodesic.rs`,
`H` = `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`. The
evidence scripts are in `phase2/c23/` (Python; they read the D13 renders in
the worktree's `target/tmp/adaptive_geodesic_2d_g1/` and reuse the C15
raster replay, `phase2/c15/march.py`, 0.1 mm cells).

- `seq.py`: the SVG in draw order (= emission order): wraps, cuts, links,
  rapids, entry rings, in world mm (the harness `View`: min x 5, max y 55,
  scale 852/70).
- `model.py`, `ends.py`: the replay of the planned wraps in cutting order,
  with the planned turns; the swept width at each arc's start and end.
- `per_entry.py`: the table of task 2. `levels.py`: the level of a wrap
  from the exact geodesic distance (C15 `geom.geo`).
- `lens.py`: the arc-end lens, closed form against a raster.
- `v2.py`, `lensends.py`, `flip.py`: the v2 plan in the model.

### The pre-registered rule (lead, written before the predictions)

KEEP a v2 column (`geodesic_local_oneway_v2` against
`geodesic_local_oneway`, `geodesic_local_zigzag_v2` against
`geodesic_local_zigzag`) when ALL four hold, else DROP:

1. Its pocket entries are below its base column (< 41 one-way, < 44
   zigzag), and its star entries are not above the base (≤ 34, ≤ 38).
2. No step that is not a turn is over the ceiling 0.3626 (the residue mop
   excepted). Read: `steps on no turn (emitted)` and the `over:` lines.
3. Every turn is fed: `turns with the reduced feed N, without 0`.
4. Pocket C = 0, and `clearing wrap` cascades ≤ the base (5 one-way, 10
   zigzag).

The `_headon` columns are attribution columns; the rule does not judge
them.

### Problem 2: the plunges at the island's lower right — CAUSE FOUND

**What each entry is.** In the one-way render the entries #7 to #17 (in
emission order) lie on the grown island (12.0 to 13.7 mm from its centre).
For each one, the cut before it stops 0.6 to 2.1 mm short of the end of a
lower-lobe arc. The entry is at that arc's end, and the next segment is
the keep-down return along the arc. The readout names the refused steps:
the last wrap steps into the island, `clearing wrap`, intrinsic, read 0.45
(for example (37.02, 16.50) → (37.06, 17.00) 0.4540; (38.91, 16.00) →
(38.97, 16.50) 0.4506; (40.86, 16.00) → (40.91, 16.50) 0.4500).

| Entry | At (mm) | Arc (wrap) | Cut stops before the arc end | Model reading at the end | Model run over c before the end | Lens (closed form), θ |
|---|---|---|---|---|---|---|
| #7 | (39.00, 16.91) | w11 | 1.51 mm | 0.674 | 1.7 mm | 0.672, +1.2° |
| #8 | (40.91, 16.50) | w12 | 1.01 | 0.658 | 1.4 | 0.734, +8.8° |
| #9 | (44.50, 17.31) | w14 | 1.50 | 0.613 | 1.7 | 0.757, +13.5° |
| #10 | (47.35, 18.50) | w16 | 2.06 | 0.707 | 1.9 | 0.714, +11.3° |
| #11 | (48.62, 19.50) | w17 | 0.78 | 0.585 | 1.1 | 0.776, +17.6° |
| #12 | (49.79, 20.50) | w18 | 0.95 | 0.705 | 1.6 | 0.748, +14.8° |
| #13 | (51.55, 23.00) | w20 | 1.71 | 0.634 | 1.1 | 0.799, +21.1° |
| #14 | (52.17, 24.50) | w21 | 0.61 | 0.706 | 1.9 | 0.773, +17.3° |
| #15 | (53.00, 25.83) | w22 | 0.94 | 0.595 | 1.7 | 0.752, +14.7° |
| #16 | (53.50, 28.82) | w24 | 1.32 | 0.628 | 1.6 | 0.718, +11.7° |
| #17 | (53.50, 31.92) | w26 | 2.05 | 0.636 | 1.7 | 0.752, +14.6° |

(`per_entry.py`. "Model": the 0.1 mm raster replay of the planned wraps
with every earlier wrap and turn cut. The planner reads the same steps at
0.45 on its 0.5 mm lattice; the arc ends lie up to one cell diagonal short
of the wall, so the planner does not see the full lens.)

**The mechanism: the head-on arc end.**

- Behind the island the levels are circle involutes (C15 (a)). An involute
  meets its base circle square. So each lower-lobe arc runs into the grown
  island head-on.
- At a wall that an arc meets head-on, the arc one level before leaves a
  lens of stock between its own end and the wall. The cutter that runs
  into the wall on the next arc cuts that lens in its last 1 to 2 mm. The
  closed form (R:`arc_end_lens`; derivation in its doc; the unit test
  checks it against a raster within 0.0083): `(R + Δ/2 + h sin θ)/2R`,
  `h = √(R² − Δ²/(4 cos² θ))`, with the cases where the discs do not meet.
  Head-on (`θ = 0`) it is `(R + Δ/2)/2R`: 0.62 to 0.66 at Δ 1.5 to 1.9.
  Every value of `Δ` reads over the ceiling.
- The one-way climb puts the island at the END of every lower-lobe arc
  (the stock is on the right of a travel from the bottom wall to the
  island). The cutter returns along the arc from its end, so the walker
  re-enters at the end of the refused piece: the entry.
- On the upper side the climb puts the island at the START of every arc.
  There the counted turn cuts the same lens (turn swept 0.78 to 0.97, all
  fed, D13), and the upper side has 2 island entries against 11.
- Only every second lower-lobe arc has an entry (w11, w12, w14, w16, ...).
  The re-entry stamps `R` plus the helix radius at the end it plunges, which
  cuts most of the next arc's lens.

**The hypotheses the lead named, against the evidence.**

- Turn over the bound at the convex wall: no. No join at the island's
  lower right is over the bound, and none is `outside` (the `.c16.txt`
  `over:` list has none there). The turns of the lower lobe lie at the
  bottom wall (26.06 to 64.89, y 8), 1.49 to 2.89 mm.
- The return link refused: a consequence, not the cause. The return starts
  at the planned arc end, inside the lens that the refused steps left.
- The one-way order puts the arc start at the island: the reverse. The
  climb puts the arc END at the island on the lower side, and that is the
  side with the entries.
- Zigzag: every second lower-lobe arc ends at the island; the render shows
  10 entries there (#6 to #15), and 8 more on the upper side (#18 to #25),
  where the alternate arcs also end at the island.

**Why the other walls pass.** The seed circles meet the bottom wall
obliquely, `θ` −12° to −42° (the travel leans away from where the next
arcs end). The closed form reads 0.41 to 0.53 there and the model 0.40 to
0.51, and the zigzag render cuts every second lower-lobe arc into the
bottom wall with no entry there. So the planner passes an oblique end and
refuses a head-on one; the closed form ranks the two ends the same way.

**The fix (derived; `ArcEnds::HeadOn`).** An arc starts at the end that
reads the larger lens, and ends at the other.

- Per open arc, the producer reads `arc_end_lens` at both ends
  (R:`end_lens_at`): the travel over the last pass step, the wall normal
  (R:`wall_normal`: an end within one cell diagonal of a ring of the
  tool-centre region, the C14 `turn_bound` argument), the sign of `θ` from
  the side where `T` grows (R:`right_growth`, the sum of
  `climb_oriented`), and `Δ` from the level step.
- When one end reads more than the other by more than `cell / D` (0.083,
  the planner's reading quantum; a smaller difference is below what the
  planner resolves), the arc starts at that end. Else the default holds:
  climb for one-way, the end nearer the cutter for zigzag. On the star the
  two ends of an arm's arc read alike (mirror walls), so the default holds
  there.
- Why it removes the plunge: the counted turn at the start cuts the lens
  (as on the upper side today), and the arc ends at the oblique wall, where
  the approach reads less.
- The cost: on the one-way column the reversed arcs are cut conventional,
  not climb. Within a lobe every arc still runs one way. The operator
  rules on this.
- Zigzag: an arc that the rule starts at the wall where the arc before
  started cannot be reached by a turn from the cutter. Such a join takes
  the one-way return and the turn from the start (`zigzag_returns`), when
  that turn fits. So in a lobe with a head-on wall the zigzag column runs
  one-way.
- The order (one-way and the ruled zigzag arcs): the distance to a
  candidate is from the end of the arc just cut or from its start (the
  return), to the candidate's real start (the ruled start, or the climb
  start). Before, the one-way pick measured from the arc's end to the
  candidate's nearer end, though the cutter stands at the start after the
  return.

Model (`v2.py oneway 1 1`): every lower-lobe arc w8 to w28 starts at the
island. The island turns are 1.44 to 2.47 mm. The bottom-wall ends read
0.36 to 0.51 (the zigzag render passes these). The first 3 mm after the
turn read up to 0.47 on w28 and 0.41 to 0.42 on w14, w23, w24: the same
range as the upper-lobe starts today, of which #22 (0.43) refused.

### Problem 1: the far top-right corner — CAUSE FOUND

The corner lies behind the island, between the meeting line, the right
wall and the top wall: levels 28 to 36 (`levels.py`). Under
`MeetingRule::Split` each level there is two arcs: a lower branch L (right
or bottom wall → meeting line) and an upper branch U (meeting line → top
wall).

**The order and the joins (one-way, `order.py`, entries #23 to #31).**

- The order cuts the V of each level in turn: U28, L28, L29, U29, U30,
  L30, L31, U31, ... (the nearest child is the other branch at the meeting
  line). The climb starts every L at the right wall and every U at the
  meeting line.
- U → L at the top wall: the next L starts at the right wall, 32 to 45 mm
  away: a link and an entry (#23, #24, #27, #30).
- L → L along the right wall: the lower front sweeps along the right wall
  and round the bottom-right corner, so consecutive L starts lie 3.8 to
  16.9 mm apart (θ 53° to 66°, `.c16.txt`): over the bound, an entry (#25,
  #28).
- U ends at the top wall: the U arcs meet the top wall near square
  (θ −16° to −25°, lens 0.45 to 0.54; model 0.42 to 0.52 over the last 0.3
  to 1.2 mm), the head-on class of problem 2 (#26, #29).
- #31 is the mop. Zigzag has the same three kinds (#27 to #31).

**The meeting line itself is not the cause.** The V vertex holds the
ceiling in every model (C15 (a): interior 0.3574 with 9 sharp V; this
model with the merged V: interior 0.333 to 0.358). The cost of the split
is the number of arcs and joins: two arcs per level, each with an end at
the meeting line and the other at a different wall.

**The fix: merge, then the head-on rule.**

- The operator's queued idea ("merge the two segments at the meeting line
  into one clean arc") is `MeetingRule::Through`: the contour runs through
  the meeting-line cells, so each level is one arc from the top wall to
  the right (or bottom) wall. It is the existing code path; with
  `Spacing::LocalLevel` the kink reads concave (its centre lies in the
  stock wedge), so the level steps do not change (`front_radius` gives `s`
  there).
- Alone, the merge does not help one-way: the climb starts each merged arc
  at the right wall, where the starts are 3 to 17 mm apart.
- With the head-on rule: the top-wall end reads 0.48 to 0.54, the
  right-wall end 0.31 to 0.36 (`lensends.py`). So the merged arcs of levels
  30 to 35 start at the top wall, where the ends lie 2.07 to 2.11 mm apart
  (all turns), and end at the right wall, which reads ≤ 0.356 in the model.
  Levels 28 and 29 (ends at the bottom-right corner) start at the bottom.
- C15 (c) rounded the V with a fillet `r ≤ 2.68 mm` (the largest that holds
  the gap at the vertex). A fillet is not built: any radius under that
  bound holds, so a choice inside it would be a tuned shape, and the sharp
  V holds the ceiling already. If the operator wants the corner rounded
  for the machine, `r = 2.68 mm` is the derived value.

Model of the corner (`v2.py`, levels 28 to 36): 3 joins over the bound (the
entry into the region, levels 29 → 30 where the start side changes, and
the last short arc) against 8 clearing entries now (#23 to #30).

### Code (C23)

- R: `ArcEnds { Default, HeadOn }` and `GeodesicOptions::ends` (G1 and C2
  set `Default`; every earlier preset inherits it). Presets
  `C23_ONEWAY_HEADON`, `C23_ZIGZAG_HEADON` (the KEEP columns plus the rule;
  the attribution columns) and `C23_ONEWAY_V2`, `C23_ZIGZAG_V2` (plus
  `MeetingRule::Through`).
- R: `arc_end_lens`, `wall_normal`, `end_lens_at`; `climb_oriented` split
  into `right_growth` (the same sum; byte-identical).
- R `ordered_wraps`: under `HeadOn` (and arc links on) the ruled start per
  open chain, the climb start for the pick, the pick distance from the end
  or the start of the arc just cut, and the mark `Wrap::head_on`. With
  `Default` every value is as before.
- R `geodesic_segments` turn block: `via_return` (one-way always; zigzag
  under `HeadOn` when the step from the cutter does not fit and the one
  from the start does). With `Default` the decision is the old one.
- R `ArcReport`: `head_on_set`, `head_on_reversed`, `zigzag_returns`.
- R module doc: a section "The head-on arc end and the merged meeting
  line (C23)".
- R unit tests: `the_arc_end_lens_matches_a_raster_of_the_end` (21 cases,
  both branches of the form; head-on equals `(R + Δ/2)/2R`),
  `the_wall_normal_points_into_the_nearest_wall` (box wall, island, open),
  `the_head_on_rule_keeps_every_turn_bounded_and_no_other_step_over`
  (`C23_ONEWAY_HEADON` and `C23_ZIGZAG_V2` on the island pocket: the rule
  sets some starts, every turn ≤ the bound, `unfed` 0, 0 clearing steps
  over).
- H: the four columns after `geodesic_local_zigzag`; the module doc; in
  `c16_text`, the line `head-on rule piece i: ...`, printed only when a
  count is non-zero, so every earlier `.c16.txt` is unchanged.

### Predictions (NOT results; the run decides)

- **Controls byte-identical**: every `.txt`, `.svg`, `.png` and `.c16.txt`
  of D13 (`summary.txt` gains the four new blocks only). The KEEP columns
  `geodesic_local_oneway` 34 / 41 and `geodesic_local_zigzag` 38 / 44 are
  unchanged.
- **Pocket `geodesic_local_oneway_v2`**: the 11 island entries (#7 to #17)
  fall to 0 to 3; the corner's 8 clearing entries fall to about 3. Mop
  entries can move (the island band). Entries 24 to 32. `head-on rule`:
  about 25 to 35 arcs set, 20 to 30 against the climb (the lower lobe and
  the corner). Return links stay about the same mm (the merged arcs are
  longer, the lower lobe is unchanged in length).
- **Pocket `geodesic_local_zigzag_v2`**: the island entries on both sides
  (#6 to #15, #18 to #25) and the corner's fall; `zigzag_returns` 30 to 50
  (most of the pocket runs one-way). Entries 22 to 32. Keep-down travel
  rises toward the one-way column's (about 600 to 1000 mm).
- **Pocket `_headon` (Split kept)**: the island entries fall as in v2; the
  corner changes (the U arcs start at the top wall, the L arcs at the right
  wall) but its over-bound L joins stay. One-way 28 to 36, zigzag 26 to 36.
- **Star (all four)**: the arc ends of an arm read alike, so few arcs are
  ruled. The one-way pick now measures to the climb start from the end or
  the start of the last arc; that can change the order. Entries within ±4
  of the base (34, 38). This is the risk for rule (1).
- **Rule (2)** holds by construction (the replay admits nothing new).
  **Rule (3)**: `without 0` expected; a turn at a masked island cell can
  read as a full-feed turn, as at D13. **Rule (4)**: C 0 likely; wrap
  cascades ≤ base likely (fewer refused wrap steps).
- **Risk: `outside`.** The island turns of the reversed lower lobe join
  ends on the grown island, 1.4 to 2.5 mm apart; in the model their chords
  pass 12.998 to 13.317 mm from the island centre (the grown island is
  r 13; the SVG holds 0.008 mm), so two of them touch the edge. The upper-lobe turns
  of the same kind had 0 `outside` at D13. A join that is `outside` stays a
  link (an entry); the `arcs piece` line counts it.
- **Verdict (predicted): KEEP for one-way v2 (medium confidence), KEEP for
  zigzag v2 (medium), with the star as the risk.** The look decides after
  the rule; the reversed (conventional) arcs need the operator's word.

### Commands for the verifier (C23; D13's 2D list)

Before the run, copy D13's render dir (for the byte compare). Then, from
the worktree, through the lane script, each with a `timeout`:

```text
cp -r target/tmp/adaptive_geodesic_2d_g1 <scratch>/d13_renders
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
for f in <scratch>/d13_renders/*.txt <scratch>/d13_renders/*.svg <scratch>/d13_renders/*.png; do cmp -s "$f" target/tmp/adaptive_geodesic_2d_g1/"$(basename "$f")" || echo "DIFF $f"; done
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
```

`path.rs`, `search.rs` and `adaptive3d/` are not changed by C23, so the
pass-load sentries and the 3D sentries are not needed for C23 (C22's own
list covers its files). The C12 test is not in this list.

Read in this order:

1. The byte compare: only `summary.txt` differs (by the four new blocks).
   Any other difference is a defect of the threading: stop and report it.
2. The three new unit tests.
3. For each v2 column, shape by shape: `entries (plunges)`, `entries by
   producer`, `arcs piece 0` (turns, over the bound, outside, return
   links), `steps on no turn (emitted)`, `clearing turns (emitted)`
   (`without`), the cascade block (`clearing wrap` cascades), and the
   `.c16.txt` `head-on rule` line.
4. The pocket entries by place against D13: the island's lower right
   (x 36 to 55, y 15 to 33), the island's upper side, the corner
   (x > 50, y > 34).
5. Apply the rule above. Then the `_headon` columns for the attribution
   (how much of the change is the rule alone, how much the merge).

### Files changed by C23

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs` (outside the C12
  block).
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`.
- New: `phase2/c23/*.py` (the evidence scripts).
- This note.

## C25 — zigzag turns lost to a lift and a plunge (2026-10-25)

Status: DIAGNOSIS COMPLETE; NO CODE CHANGE (no cargo; no commit). Base
31935b89. The fix is the C23 rule, which the 3D arm does not use yet. The
change that applies it lies in C22's files, so it is a lead decision
(below).

Inputs: `phase3/fix2/turn_probe_output.txt` and D-FIX2 (`P15_D_verify.md`).
In the one-level C12 test, `geodesic_local_zigzag` plans 46 turns and cuts
27 as turns. For each of the 19 others, the turn's start is the target of a
`Rapid` in the cleaned 2D plan, and 17 of the 19 start 11.27 to 12.01 mm
from the island centre (the island of radius 5 grown by the 2R inset is
r 11). The heavy test: 95 planned, 56 cut.

### Which step drops them

The candidates the lead named, in turn:

- **The head-on rule (C23): no.** The 3D arm maps `geodesic_local_zigzag`
  to `GeodesicOptions::C20_LOCAL_ZIGZAG` (`adaptive3d/mod.rs`,
  `research_arm_override`; the C12 test lines 824 and 907). That preset has
  `ends: ArcEnds::Default`, so the rule is off in every 3D run so far.
- **The F-038 filter: no.** F-038 / F-038b in `adaptive3d/mod.rs` gate a
  region by its forecast cut length and the keep-down link reach. The probe
  sees the `Rapid` in the cleaned 2D plan of the slice, before any of that.
- **The clip: no.** The turn's end is a cut point, and the turn is cut (as
  the first step out of the plunge).
- **The replay refusal: yes.** It is the C23 problem 2 mechanism, the
  head-on arc end. A zigzag arc that ends square into the island (a circle
  involute meets its base circle square) meets the lens of stock that the
  arc one level before leaves at the wall (`arc_end_lens`, `(R + Δ/2)/2R`
  head-on, over the ceiling for every Δ). The replay refuses the last 0.6
  to 2.1 mm of the arc. The walker re-enters after the refused step, which
  is the arc's end and so the zigzag turn's start. The turn is then the
  first step out of a plunge: the 2D replay and the C20 3D departure rule
  read it as a departure, not as a turn.

**The same pattern in 2D, measured** (`phase2/c25/turnentry.py`: a turn
whose start is an entry ring of the render, within 0.05 mm; the D14
renders):

| Column | Turns emitted | Entries | Turns that start at an entry | Where |
|---|---|---|---|---|
| pocket `geodesic_local_zigzag` | 59 | 44 | **20** | 19 on the grown island (13.03 to 13.56 mm from its centre, grown radius 13); 1 at the top wall of the far corner (head-on, C23 problem 1) |
| pocket `geodesic_local_zigzag_headon` | 51 | 34 | 3 | the far corner (the split U arcs at the top wall) |
| pocket `geodesic_local_zigzag_v2` | 53 | 26 | **0** | — |
| star, all three | 45 | 38 | 0 | — |

The C12 pocket has the same geometry: an island in a rectangle, the front
round both sides, a meeting line behind. 17 of its 19 lost turns lie on the
grown island, and the other 2 (24.7 and 27.7 mm out) match the 2D far-corner
kind.

### The fix

- **The rule that removes the cause is `ArcEnds::HeadOn`, with the merged
  meeting line: `GeodesicOptions::C23_ZIGZAG_V2`** (C23; KEEP at D14,
  star 38 / pocket 26). Each arc starts at the end with the larger lens.
  The counted turn cuts the lens there, at depth, under the turn rule
  (length ≤ s + (1 + √2)·cell, feed reduced by the measured engagement).
  The arc ends at the oblique wall, where the approach holds. Where the
  step from the cutter does not fit, the zigzag join takes the keep-down
  return through the band just cut and the turn from the arc's start: the
  keep-down fallback the lead asked for.
- **Why not a fix inside plain zigzag.** In plain zigzag the lens is
  real stock between the accepted cut and the wall. A keep-down link from
  the stop point to the turn start crosses it, and the replay refuses it,
  as it should. A turn that leaves the arc before the wall leaves the lens
  as residue (C23 §problem 2). An approach exemption would widen the
  operator's turn ruling, so I do not build it.
- **The change is one preset per place, in C22's files** (I do not edit
  them; the lead decides):
  - `adaptive3d/mod.rs`, `research_arm_override`: a new name, for example
    `"geodesic_local_zigzag_v2" => Some(ClearingStrategy3d::Geodesic(GeodesicOptions::C23_ZIGZAG_V2))`.
    Or map `geodesic_local_zigzag` to `C23_ZIGZAG_V2`, which changes the
    meaning of that name.
  - `tests/adaptive3d_geodesic_arm_c12.rs`: the same preset in the mode
    lists at lines 824 and 907 (or a fifth mode).
- **No change to `research_geodesic.rs`.** `C20_LOCAL_ZIGZAG` stays as it
  is, so the 2D KEEP control `geodesic_local_zigzag` stays byte-identical.
  The 2D `geodesic_local_zigzag_v2` (star 38 / pocket 26) does not change
  at all. The rule reads the slice's own tool-centre region
  (`geodesic_segments` builds it per call), so it applies in 3D slices with
  no threading.

### Predictions (NOT results; the run decides)

For the C12 tests with the zigzag mode on `C23_ZIGZAG_V2`:

- **One-level test:** the turns that start at a re-entry fall from 19 to
  0 to 2 (2D pocket: 20 → 0). Planned turns 40 to 48 (the head-on lobes run
  one-way, so some zigzag joins become return + turn; 2D pocket 59 → 53).
  Cut turns = planned minus 0 to 2. Entries 68 → 45 to 58 (2D pocket
  44 → 26). Keep-down length rises (the returns; 2D pocket 211 → 998 mm).
  Unfed 0.
- **Heavy test (two levels):** uncut 39 of 95 → 0 to 4. 0 over the
  screen and the exact bound (the rule changes the order and the
  orientation only; every step still goes through the replay).
- **2D star and pocket:** no change (no code change): `zigzag_v2` 38 / 26,
  and every other column byte-identical to D15.
- **Risk:** the same as C23: a turn whose straight step touches the grown
  island counts as `outside` and stays a link. The 2D v2 pocket had 0.

### Commands for the verifier (after the lead's one-preset change)

```text
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_geodesic_arm_c12 -- --nocapture
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture
python3 planning/adaptive_research_2026-10-23/phase2/c25/turnentry.py pocket_geodesic_local_zigzag_v2 star_geodesic_local_zigzag_v2
```

Read the zigzag mode's turn line (planned, cut, fed, unfed), the entries,
and the probe's `from=rapid_target` count if the probe runs again.

### Files

- New: `phase2/c25/turnentry.py` (reads the D14/D15 renders in the
  worktree; it uses `phase2/c23/seq.py`).
- This note.

## C24 — tight pass spacing in 3D (Corne "3D Rough 1"): measured; no spacing fix (2026-10-25)

Status: MEASURED. No Rust changed, no cargo, no commit. The cause is not
in the `LocalLevel` code, so I built no fix (the lead's rule). Inputs: the
D-FIX2 sim-artifact runs at 31935b89, `phase3/fix2/simart/corne/`
(`program.nc`, `tp_1_3D_Rough_1.json`), read with the move mapping of
`phase3/fix2/fixture3d_v2.py` (`load_op`). Scripts and their outputs:
`phase3/c24/` (`load.py`, `rays.py`, `weighted.py`, `lengths.py`;
`*_output.txt`). Each runs in seconds: `python3 weighted.py`, `python3
rays.py geodesic_local_zigzag geodesic_local_oneway`, `python3 lengths.py`.

### The premise is wrong: at s 1.2 the ceiling is lower, so ρ* is 25.4 mm, not 2.95 mm

- The ceiling comes from the stepover (`PassLoad::swept_width`):
  `c = (1 − cos(1.05·acos(1 − s/R)))/2`. At s 2 this is 0.3626, so
  c·D = 2.1759 mm. At s 1.2 it is **c = 0.2189, c·D = 1.3132 mm**.
- So `ρ* = s(2R − s)/(2(c·D − s))` = 1.2 × 4.8 / (2 × 0.1132) =
  **25.45 mm**. The lead's 2.95 mm used the s 2 ceiling.
- Inside 25.45 mm the law spaces the levels at `s'(ρ)`: 0.70 at ρ 3,
  0.87 at 5, 1.05 at 10, 1.13 at 15, 1.17 at 20, 1.20 at 25. The 3D arm
  builds its 2D params from the op stepover (`adaptive3d/clearing.rs`
  `params_2d`), so the law applies as designed.
- wanaka200 (s 2.0): ρ* = 22.7 mm, and its median 1.979 is the same
  effect at a smaller scale.

### The plan's own levels (Corne, Z 5.00, one slice piece, seed (51.05, 58.55))

Method: the level sets are circles about the seed wherever the seed sees
them (here, the whole piece). A wrap chord has both ends at one distance
from the seed and a sagitta of at most 0.5 mm. The distances of these
chords cluster into the levels (`weighted.py`).

| Arm | Levels | Median step | Wrap-length-weighted median | Step − law: min / median / max |
|---|---|---|---|---|
| zigzag | 75 | 1.200 mm | 1.200 mm | −0.278 / −0.000 / 0.000 |
| one-way | 76 | 1.200 mm | 1.200 mm | −0.278 / −0.000 / 0.000 |

No step is over the law. The plan's median spacing is the stepover.

### Counts per cause (74 steps, zigzag; one-way gives the same)

| Cause (the lead's candidates and what I found) | Steps | Effect |
|---|---|---|
| At s (ρ ≥ ρ*) | 49 | none |
| Inside ρ* = 25.45 mm, at the law (the law itself, at the s 1.2 ceiling) | 21 | steps 0.75 to 1.18 mm |
| LocalLevel ΔT bisection (the lower end of a bracket ≤ the tolerance) | the same 21 | mean −0.050 mm under the law root (bracket 0.075 mm); 0 outside ρ*, where `ΔT = 1` holds at once |
| 3-point ρ_local or a tight piece elsewhere on the level (the level's minimum read away from the arc) | 1 | the level at r 39.97: step 0.922 against the law's 1.200 (both arms) |
| Reading noise of my clustering (it differs between the arms) | 1 + 2 or 3 | an under-step next to an over-step |
| Per-slice-piece re-spacing | 0 | one piece holds all levels at Z 5.00 |
| The 3D lift | 0 on spacing | XY is not moved; see the side finding on the RDP |

With G-code alone I cannot tell whether the one coupled level (r 39.97)
comes from a lattice misreading (`front_radius` takes a sample off the
region as convex: the risk that C16 recorded) or from a truly tight piece
of that level elsewhere in the slice. It is 1 level of 75.

### Why the ray medians read 1.125 (zigzag) and 0.960 (one-way)

The D-FIX2 rays start at the seed itself (the ring nearest the AgentSearch
entry is the geodesic seed). So they over-sample the levels inside ρ*.
The piece is only about 35 mm deep in three of the four directions.
`rays.py` reproduces the D-FIX2 counts (179 and 216 gaps):

- Zigzag: 105 of 179 gaps are inside ρ* (median 1.050, the law) and 74
  are outside (median **1.200**). By kind: 131 are wrap-to-wrap at the
  law. 25 involve a cut that is not a wrap: the starter helix gives the
  0.09 mm minimum at t 2.85, and there are wall-pass and residue moves.
  23 are wrap pairs that read ±0.13 to 0.20 mm around 1.2. Each pair sums
  to 2.4 mm: the ray crosses one level mid-chord, where the chord lies
  under the circle by its sagitta (see the RDP below).
- One-way: 70 of 216 gaps involve a **straight move across the cut**.
  Such a move has both ends on one level circle and a sagitta over
  0.5 mm, for example (69.43, 94.05) → (88.10, 43.55): 53.8 mm, both
  ends at r 39.97. These moves are fed moves through cleared stock.
  The trace does not tag them as keep-down links, so the page reads
  them as cuts. At Z 5.00 they are 1940 mm, or 22% of the one-way cut
  length at that level. They are not passes, so they are not spacing.
  They come from the C22/C23 return links, not from `LocalLevel`.

### Where the cycle time goes (the first rough op, cut XY length, `lengths.py`)

| Arm | Z 17 | Z 11 | Z 5 | Z 0.5 | Sum | Run total (both ops) |
|---|---|---|---|---|---|---|
| AgentSearch | 6646 | 2593 | 2603 | 2470 | 14312 mm | 2112 s |
| geodesic zigzag | 13469 | 6571 | 6668 | 6608 | 33316 mm | 2745 s |
| geodesic one-way | 16365 | 8774 | 8857 | 8765 | 42761 mm | 3068 s |

- At Z 5.00 the zigzag wraps are 1804 mm inside ρ* and 3995 mm outside.
  The other cuts are 869 mm.
- The law adds about 2.2 levels inside ρ*: 20 levels from r 3 to 24.4,
  against 17.8 at plain s. That is about 200 mm of 6668 mm at Z 5.00,
  about 3%.
- The gap to AgentSearch is the AgentSearch spacing, not the geodesic
  spacing. AgentSearch spaces its passes at 2.55 mm, which is 2.1 times
  its stepover of 1.2, and 20% of its cut length is over the bound
  (D-FIX2). The geodesic arm spaces its passes at the stepover.

### Proposed fix and predictions (NOT results)

1. **Spacing: no change.** The plan is at the law. The spacing is not
   too tight; the s 1.2 ceiling requires it. Predicted median level step:
   **1.200 mm** (as measured). Predicted cycle time: **unchanged**,
   2745 s (zigzag) and 3068 s (one-way).
2. **Not recommended:** take the upper end of the bisection bracket. It
   would put steps up to 0.075 mm over the rule. The gain is about 1
   inner level per slice, under 1% of the cut length.
3. **Not recommended:** plain s inside ρ*. Prediction: about −3% of the
   wrap length at each level, which is about 2745 → 2700 s (−1.5%). It
   breaks the ceiling inside ρ* by construction.
4. **The page's measurement (the lead's call):**
   - Measure the step between consecutive levels along the normal
     (`weighted.py`), or weight the gaps by wrap length.
   - Leave out the starter helix and moves that are not wraps.
   - Read a one-way straight move across the cut as a link, not a pass.
5. **The real lever is the op stepover** (an operator setting). The law
   scales with it. Wider passes need a larger `s` (and then a higher
   ceiling), not a change to the law.

### Side findings (not mine to fix; for the lead)

- **One-way straight moves across the cut:** 1940 mm per level at Z 5.00
  (22% of that level's cut length) are fed through cleared stock. They
  are probably most of the one-way runtime cost over zigzag (+323 s).
  Owner: C22/C23 (the return links in 3D).
- **The 3D RDP chords:** the 3D emission simplifies at
  `cut_simplify_tolerance` = max(tolerance 0.1, `segment_merge_tolerance`
  0.3) (`adaptive3d/mod.rs:259`). The wraps at r ≈ 60 come out as 7 to
  12 mm chords with sagitta up to 0.29 mm. Example: (110.554, 78.550) →
  (113.247, 67.050), 11.81 mm on r 62.78. Mid-chord, the next level then
  meets up to s + 0.29 = 1.49 mm, against c·D = 1.313 mm. That is a load
  matter (the C18 over-bound share), not a spacing matter. The planner
  stamps the same chords (G-PLANSIMGAP).

### Files (C24)

- New: `phase3/c24/load.py`, `rays.py`, `weighted.py`, `lengths.py`, and
  their `*_output.txt`.
- This section. No Rust file changed.

## C27 — why zigzag_v2 is worse on Corne (2026-10-25)

Status: MEASURED; NO CODE CHANGE (no cargo; no commit). Data: the D16b
Corne runs, `phase3/fix2/corne/geodesic_local_zigzag{,_v2}/` (both at
4cceca914 for v2 and 31935b89 for zigzag; `program.nc` and
`tp_1_3D_Rough_1.json`), op "3D Rough 1". Scripts in `phase3/c27/`; each
loads the op with the page's own loader (`fix2/fixture3d_v2.py`
`load_op`: move i = G-code line i + 1, the level from Z, the classes from
the semantic trace).

### Per level (`perlevel.py`, `arcs.py`)

| Level | Entries zigzag / v2 | "Cut" mm zigzag / v2 | Keep-down mm zigzag / v2 | G2/G3 moves in v2 (count, arc length) |
|---|---|---|---|---|
| Z 17.00 | 10 / 14 | 13469 / 15318 | 1392 / 1824 | 231, 1949 mm |
| Z 11.00 | 6 / 8 | 6571 / 8630 | 262 / 263 | 50, 2061 mm |
| Z 5.00 | 6 / 8 | 6668 / 8714 | 322 / 314 | 50, 2061 mm |
| Z 0.50 | 34 / 37 | 6608 / 8620 | 49 / 48 | 135, 2018 mm |
| Sum | 56 / 67 | 33316 / 41281 | 2026 / 2450 | 466, 8089 mm (zigzag: 17, 53 mm) |

The "cut" column is the page's class: every fed move that no trace span
tags as a keep-down link, measured by its chord.

### Where the extra cut length comes from: the keep-down returns

- **The swept area is the same.** The disc R 3 along the fed cut moves
  covers the same area in both arms at every level, within 1 mm²
  (`cover.py`: 13926 / 13925, 9465 / 9465, 9355 / 9355, 8605 / 8605 mm²).
- **The arcs are the same.** Fed G1 cut length of one arm that runs more
  than 0.3 mm from every G1 cut move of the other arm: 124 to 228 mm per
  level each way (`newgeom.py`). These are the joins that differ, not the
  wraps.
- **The extra length is G2/G3 moves.** In v2 these are the one-way returns:
  each runs from an arc's end back to its start (for example moves 8020 to
  8028 at Z 5.00: the turn (99.56, 43.55) → (100.81, 43.55), the arc up to
  (89.47, 93.55), then one G2 back to (100.81, 43.55), then the next turn).
  At 4cceca914 the 3D arm held each 2D return `Link` as a `Cut` (C22), so
  the arc fitter joined a return into one G2 and the trace tags it as a
  clearing cut. Their arc length is 8089 mm. The cut difference is
  7965 mm (C26 now emits them as keep-down links; they stay travel at the
  level and still cost time).
- **The page images draw each G2 as its chord.** The diagonal blue lines
  in `corne_geodesic_local_zigzag_v2_*.png` are these returns, not cuts
  across the stock.
- So the +209 s of cycle time is mostly the return travel: 8089 mm over
  209 s is 2320 mm/min, a feed rate, not extra machining.

### Where the extra entries come from

Rings matched between the arms within 1.5 mm (`rings.py`):

| Level | Rings only in v2 | Rings only in zigzag | Net |
|---|---|---|---|
| Z 17.00 | 5 | 1 | +4 |
| Z 11.00 | 3 | 1 | +2 |
| Z 5.00 | 5 | 3 | +2 |
| Z 0.50 | 5 | 2 | +3 |

- Every v2-only ring lies on a wall: (99.56, 43.55) at three levels (the
  bottom wall of the right-hand region), (15.55, 66.48) and (15.05, 35.73)
  (the left wall), (33.26, 91.45), (103.77, 90.00) and (133.52, 89.39)
  (the top wall), and at Z 17 (62.14, 14.05), (54.05, 12.55),
  (109.04, 31.55), (144.05, 31.71) and (149.05, 50.12).
- Each is the start of an arc. The next fed move runs from it into the
  arc (0.9 to 3.8 mm), or it is the first turn of a lobe. No v2-only ring
  lies on a return (none mid-arc), so no return was refused.
- **By cause:**
  - **The head-on rule: all 11 net.** It moves the start of an arc to the
    end with the larger lens. Where the cutter does not stand at that end
    when the lobe begins, the lobe begins with a link that the replay
    refuses, and an entry. Plain zigzag starts each lobe at the end
    nearer the cutter.
  - **The keep-down return fallback: 0 entries, all the extra length.** It
    is the rule's consequence in zigzag: an arc that starts at the wall
    where the arc before started is reached only by the return.
  - **The merged meeting line (`Through`): no measured effect.** The cut
    geometry differs by at most 228 mm per level, and all of it is at
    joins. The Corne slices at Z 17, 11 and 5 have no island (the images
    `corne_*_{0,1}.png`), so no two fronts meet behind one. Z 0.50 holds a
    grid of small features with an entry each in both arms (34 and 37).
  - **Something else: none found.**

### Why Corne differs from the C12 fixture and the 2D pocket

- **The premise of the rule does not hold on Corne.** The rule reorients an
  arc so that it does not END at a head-on wall, because there the replay
  refuses the approach (C23: 11 one-way entries at the pocket island; C25:
  19 lost zigzag turns at the C12 island). On Corne the old zigzag ends
  arcs head-on at the bottom wall of the right-hand region (y 43.55, arcs
  nearly square to it) and at the top wall, and it has NO entry there
  (its rings at Z 5: (51.0, 58.5) the starter, (76.5, 32.6), (83.4, 96.5),
  (15.1, 52.9), (137.9, 45.0), (26.0, 12.7)). The replay passes those
  ends. So the rule pays the returns and the lobe-start entries and gains
  nothing.
- **What differs (measured and derived, not run):**
  - The islands: the C12 fixture and the 2D pocket each have a convex
    island, and every refused end there is on the island (C23 table, C25
    probe: 17 of 19). Behind an island the levels are involutes, square to
    it; a convex wall curves away from the end disc of the arc before, so
    the lens is larger than at a straight wall. The Corne slices at Z 17,
    11 and 5 are one outline with no island.
  - The stepover: Corne's op has s 1.2 (ceiling 0.219), the fixtures s 2.0.
    The depth of the lens along the travel at the arc's middle line is
    `R − √(R² − Δ²)`: 0.25 mm at Δ 1.2, 0.68 mm at Δ 1.9. The planner's
    arc ends lie up to one cell diagonal (0.71 mm) short of the wall. I
    did not derive whether that shortfall hides a 0.25 mm lens from the
    planner's reading; it is a candidate, not a result.
  - Not the cause: the outline pieces per slice (the swept areas are the
    same, and the extra entries are lobe starts on one outline), and the
    case wall shape as such (Corne's walls are straight; the closed form
    of `arc_end_lens` reads 0.6 head-on at Δ 1.2 on a straight wall, so
    it predicts a refusal that the planner does not make).
- So the closed form of the lens is a correct geometric reading but not a
  correct predictor of the planner's refusal on every part. The rule acts
  on the prediction; it should act on the refusal.

### Proposal: a rule for when each variant applies

**Plan both, keep the one the replay prefers, per outline piece.** The
producer plans the piece with plain zigzag and with zigzag_v2. It walks
both through the replay model it already has (`ReplayModel`, C9). It keeps
the plan with fewer re-entries. On a tie it keeps the one with less
keep-down length (the returns). No constant enters: both quantities are
counts and lengths of the producer's own plans. The rule acts on the
refusal itself, not on the closed form.

- Why per piece: a slice can hold an island piece and a plain piece; each
  takes its own variant.
- Cost: two plans and two replays per piece (the 2D harness runs about
  14 s for every column; one piece takes a fraction of that).
- A narrower form (reorient only the arcs whose default end the replay
  refuses, then plan again) needs a second planning pass with the refused
  arcs marked. It keeps zigzag everywhere the replay passes. I recommend
  the per-piece choice first; it is simpler, and it gives the bound for
  the narrower form.

**Predictions (NOT results):**

| Case | Variant kept | Entries | Keep-down / returns | Cycle |
|---|---|---|---|---|
| Corne "3D Rough 1", each of the 4 levels | plain zigzag (v2 has more entries at every level) | 56 (= zigzag) | 2026 mm (= zigzag) | 2745 s (= zigzag; cut 33316 mm) |
| wanaka200 back rough | either (the plans are byte-identical) | 22 | 3423 mm | 3950 s |
| C12 one-level test | zigzag_v2 (48 against 68) | 48 | as v2 | — |
| C12 heavy test | zigzag_v2 at both levels | as v2 (not cut 1) | as v2 | — |
| 2D star | tie on entries and keep-down (38, 251.2 mm): plain zigzag, byte-equal to v2 | 38 | 251.2 mm | — |
| 2D pocket | zigzag_v2 (26 against 44) | 26 | 998.1 mm | — |

The 2D `zigzag_v2` star 38 / pocket 26 do not get worse. Risk: the
producer's replay count can differ from the 3D lift's entries (F-038 drops
short groups; the 3D lift adds entries at air runs). On Corne the 2D
counts per piece are not in the trace, so the per-level choice above
assumes they follow the 3D entries. The run decides.

### Files

- New: `phase3/c27/perlevel.py`, `retrace.py`, `retrace2.py`, `returns.py`,
  `cover.py`, `rays.py`, `newgeom.py`, `arcs.py`, `rings.py`, `diag.py`,
  `zoom3d.py`.
- This note.

## C28 — per piece, keep the zigzag variant that the replay prefers (2026-10-25)

Status: DESIGN (written while D16c runs; no code until the lead's "go").
The lead approved the C27 proposal. Read first: `P3_C22_3d_turns.md` §10
(C26: `HeldLinks` only flags a link; `clear_one_region` emits it as a 3D
keep-down link; the research chord tolerance). C28 does not touch either:
it chooses between two 2D plans before any of that runs.

### The rule

For each outline piece the producer plans the piece twice:

- A: plain zigzag, `GeodesicOptions::C20_LOCAL_ZIGZAG` (Split, `ArcEnds::Default`);
- B: zigzag_v2, `GeodesicOptions::C23_ZIGZAG_V2` (Through, `ArcEnds::HeadOn`).

It runs each plan through the cleanup the run itself applies, `cleaned(..,
ReplayMeasure::SweptWidth, turns)`. That is the replay of the 2D run and of
the 3D slice (`slice_cleanup` calls the same function with the same
measure and the plan's own turns). Then it keeps:

1. the plan with fewer re-entries: the `AdaptiveSegment::Rapid` segments of
   the cleaned plan (each is a lift and a plunge; the starter is common to
   both, so it does not change the order);
2. on a tie, the plan with less keep-down length: the sum of the
   `AdaptiveSegment::Link` lengths of the cleaned plan, each from the
   position before it;
3. on a tie of both, A (plain zigzag: no returns).

No constant enters: two counts and two lengths of the producer's own plans,
and a fixed order. The kept plan's segments and turns are exactly the
variant's (`geodesic_segments` of A or of B), so per piece the pick column
plans byte-for-byte as one of the two existing columns.

Why the cleanup and not `ReplayModel` (C9), the model named in C27: the
model has no turn exemption (it predates C14), so it would refuse every
turn over the ceiling and miscount both plans. The cleanup is the real
replay, with the turn list.

### Code plan (after "go"; my functions only)

`R` = `research_geodesic.rs`. Nothing in the C12 block, the spacing
functions (C24) or C22's files changes.

- `ArcPick { Off (default), Replay }` and `GeodesicOptions::pick`; G1 and
  C2 set `Off`, so every existing preset keeps its plan.
  `C23_ZIGZAG_V2` is not changed.
- Preset `C28_ZIGZAG_PICK` = `C20_LOCAL_ZIGZAG` with `pick: ArcPick::Replay`.
  Its two variants are fixed by the rule: A = the options with `pick:
  Off`; B = A with `ends: HeadOn` and `meeting: Through`. For
  `C28_ZIGZAG_PICK`, A is `C20_LOCAL_ZIGZAG` and B is `C23_ZIGZAG_V2`
  (a unit test pins both identities).
- `geodesic_segments` becomes a dispatcher with the same signature (the C12
  block's `slice_segments` and the 2D run call it unchanged). With
  `pick: Off` it calls the old body, renamed `plan_piece`, so the plan is
  the old one. With `Replay` it plans A and B with `plan_piece`, cleans and
  scores both, and returns the kept `Produced`. If one variant does not
  apply (`None`), the other is kept; a cancel is passed on.
- The record: `PickRecord { entries_a, keep_down_a_mm, entries_b,
  keep_down_b_mm, kept: PickKept }` in `ArcReport::pick: Option<..>` of the
  kept plan (ArcReport has a `Default`, so no literal changes).
- The 3D readout (the lead's check of 2D re-entries against 3D entries):
  one `tracing::info!` per piece from the dispatcher: the piece's bounding
  box, both entry counts, both keep-down lengths and the kept variant. The
  CLI's subscriber prints `info` by default (`rs_cam_cli` `main.rs`), so a
  fixture run's `stdout.txt` holds one line per slice piece. The 2D harness
  sets no subscriber; it prints the record instead.
- The harness: the column `geodesic_local_zigzag_pick` after the C23
  columns. In `c16_text`, one line per piece only when `pick` is set:
  `pick piece i: zigzag re-entries N (keep-down X mm), zigzag_v2 re-entries
  M (keep-down Y mm); kept <variant>`. Every earlier `.c16.txt` stays
  byte-identical.
- The 3D arm's name (C22's file, the lead's change): add
  `"geodesic_local_zigzag_pick" => GeodesicOptions::C28_ZIGZAG_PICK` in
  `adaptive3d/mod.rs` `research_arm_override`, and the mode in the C12 test
  if wanted.
- Unit test: on the island pocket, `C28_ZIGZAG_PICK` keeps the variant
  with the lower count, its segments equal that variant's
  `geodesic_segments` output, and the record's counts equal the Rapid
  counts of the two cleaned plans.

Cost: two plans and two cleanups per piece, then the caller's own cleanup
of the kept plan (three cleanups in all). The 2D harness takes about 14 s
for all columns now; the pick column adds about three zigzag columns'
time. In 3D the arm's planning time per slice rises by about three times
(Corne zigzag: 10 s → about 30 s).

### Predictions (NOT results)

| Case | Kept | Entries | Keep-down |
|---|---|---|---|
| 2D star (one piece) | A (tie: the two plans are byte-equal, D14) | 38, the `.txt` equal to `geodesic_local_zigzag` | 251.2 mm |
| 2D pocket (one piece) | B (v2: 26 against 44) | 26, the `.txt` equal to `geodesic_local_zigzag_v2` except the column name | 998.1 mm |
| Corne "3D Rough 1" (after the 3D name is added) | A at every level and piece where v2 has more re-entries (C27: every level) | 56 | 2026 mm (C26 tags the links; the length is the same) |
| wanaka200 back rough | A (tie; the plans are byte-equal) | 22 | 3423 mm |
| C12 plain test | B | 48 | as v2 |

So the 2D zigzag_v2 star 38 / pocket 26 do not get worse (the pick column
is one of the two per piece). Risk (the lead's check): the 3D entries
include F-038's dropped groups and the lift's air-run entries, which the
2D count does not see. The `info` lines give the 2D counts per piece to
compare with the trace's entry spans per level.

### C28 built (2026-10-25; base 6ce373922; no cargo, no commit)

`rustfmt --edition 2024 --check` is green on the four files.

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`:
  - `ArcPick { Off, Replay }`, `GeodesicOptions::pick` (G1 and C2 set
    `Off`), the preset `C28_ZIGZAG_PICK`, and
    `GeodesicOptions::pick_variants` (plan A, plan B).
    `C23_ZIGZAG_V2` is unchanged.
  - `ArcReport::pick: Option<PickRecord>`, `PickRecord`, `PickKept`,
    `pick_score` (the `Rapid` count and the `Link` length of a cleaned
    plan), `pick_kept` (fewer re-entries, then less keep-down, then A).
  - `geodesic_segments` is the dispatcher (the same signature). With the
    pick off it calls `plan_piece`, the old body under a new name. With
    `Replay` it plans A and B, cleans both with `cleaned(..,
    ReplayMeasure::SweptWidth, turns)`, keeps one, writes the record, and
    logs one `tracing::info!` per piece ("C28 pick: ...": `bbox`,
    `entries_a`, `keep_down_a_mm`, `entries_b`, `keep_down_b_mm`, `kept`).
  - Unit test `the_pick_keeps_the_plan_the_replay_re_enters_less` (the
    island pocket): the two variants are `C20_LOCAL_ZIGZAG` and
    `C23_ZIGZAG_V2`; the record equals the two cleaned scores; the kept
    plan's segments and turns equal that variant's; and three
    `pick_kept` cases (a tie keeps A; less keep-down keeps B; fewer
    entries win over keep-down).
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the column
  `geodesic_local_zigzag_pick`, the module doc, and in `c16_text` the line
  `pick piece i: zigzag re-entries N (keep-down X mm), zigzag_v2
  re-entries M (keep-down Y mm); kept <variant>` (only when the pick is
  set).
- `crates/rs_cam_core/src/adaptive3d/mod.rs`: the arm name
  `geodesic_local_zigzag_pick` in `research_arm_override`, and its doc.
- `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs`: the mode in
  the plain test `the_geodesic_arm_feeds_its_turns_down_at_the_level`, and
  its doc.

Byte parity: every existing preset has `pick: Off`, so `geodesic_segments`
calls `plan_piece`, the old body, unchanged. Every earlier render, `.txt`
and `.c16.txt` must stay byte-identical (`summary.txt` gains the new
blocks). The pick column's plan per piece equals one of the two existing
columns' plans.

### Commands for the verifier (C28)

From the worktree, through the lane script, each with a `timeout`. Copy the
render dir first for the byte compare.

```text
timeout 600 scripts/cargo_lane.sh fmt --all -- --check
timeout 1800 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --lib research_geodesic
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_emission_byte_parity
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support -q --test adaptive3d_geodesic_arm_c12 -- --nocapture
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
```

Read: the byte compare; the `pick piece` lines (star: kept zigzag, a tie;
pocket: kept zigzag_v2); the pick column's `.txt` against
`geodesic_local_zigzag` (star) and `geodesic_local_zigzag_v2` (pocket),
equal except the column name; the plain C12 test's pick line (kept
zigzag_v2, 48 entries). For the Corne fixture run with the arm
`geodesic_local_zigzag_pick`, the `C28 pick` lines in `stdout.txt` give
the 2D re-entries per slice piece, to compare with the trace's entry spans
per level.

## C29 — score the pick by what the 3D output plunges (2026-10-25)

Status: DESIGN (no code; no cargo; no commit). Base 13667775d (C26b and
C28). Data: D17's Corne runs, `phase3/fix4/corne/geodesic_local_zigzag{,_v2,_pick}/`.
Script: `phase3/c29/spans.py` (it loads the op with the page's loader,
`fix2/fixture3d_v2.py` `load_op`, which now reads the move intents from
`simulation.json`. The C27 scripts were fixed on 2026-10-25 to pass
it: `perlevel.py` maps each C27 arm to its sim-artifact run, and every
§C27 number reproduces on the 4cceca914 and 31935b89 outputs).

### What the 3D stage does with a 2D re-entry (the shipped code, read)

A 2D `Rapid` of the slice plan goes through, in order:

1. **F-038** (`clear_one_region`): a group (a `Rapid` and its cuts up to the
   next `Rapid`) whose cut length, links included for the research arm, is
   under `min_region_cut_length_mm` is dropped, and its entry with it.
2. **`LevelSink`** (G-PHANTOMSTAMP): an entry that a later entry follows
   with no cut or link between is replaced (`replaced_entries`).
3. **The lift**: a run of a cut through cleared stock longer than the air
   crossover becomes an entry of its own (a 3D-only entry).
4. **`plan_entry`**: the entry becomes `RapidWithFloor`. It carries a
   `StayDownProof` (the conservative stock ceiling of the swept disc from
   the planner cursor) only when the XY jump from the cursor is at most
   `stay_down_mm` (`resolved_stay_down_mm`: `max_stay_down_distance_mm`,
   else `2R · LINK_MAX_DISTANCE_OVER_D`).
5. **The emitter** (`segments_to_toolpath`, F-038b `try_emit_stay_down_link`):
   the entry is a keep-down link only if the emitted previous position is
   within `STAY_DOWN_FROM_XY_EPS_MM` of the proof's `from`, the jump is
   within `stay_down_mm`, and `link_z` (the highest of the drop-cutter
   terrain along the line plus the leave, the stock ceiling plus the
   clearance, both ends and the descent floor) is at most `safe_z` and at
   most `to.z` plus the cutter length. Else it retracts, rapids and enters:
   the trace's "plunge entry" span, whether or not the column below holds
   stock.

### Measured on D17 (`spans.py`; every top-level entry span of "3D Rough 1", with the XY jump from the tool position before the span to the one after it)

| Level | zigzag (A): plunge (jump > 48 / ≤ 48), keep-down (all ≤ 48) | zigzag_v2 (B): plunge (> 48 / ≤ 48), keep-down |
|---|---|---|
| Z 17.00 | 10 (8 / 2), 529 | 14 (12 / 2), 693 |
| Z 11.00 | 6 (5 / 1), 82 | 8 (7 / 1), 63 |
| Z 5.00 | 6 (5 / 1), 78 | 8 (7 / 1), 59 |
| Z 0.50 | 34 (11 / 23), 349 | 37 (14 / 23), 443 |

`stay_down_mm` on Corne = 48 mm (6 mm tool, the default 8 × D; the two
counts split there exactly).

- **Every keep-down span has a jump of at most 48 mm**, in both arms.
- **Plunges with a jump of at most 48 mm exist (27 in each arm), and they
  are the SAME spans in both arms** (the same positions, `spans.py` list):
  the first entry of each level ((26.00, 12.72) from (20.00, 3.1),
  11.3 mm; the cursor is at the level above, so no proof matches) and
  the Z 0.50 grid of small regions, 18 to 42 mm apart (their link Z or the
  proof's `from` fails; they are not the geodesic piece's choice).
- So on D17 the difference A − B is exactly the difference in plunges with
  a jump over 48 mm: 8 − 12, 5 − 7, 5 − 7, 11 − 14.
- **The 2D counts do not see this.** The C28 lines: Z 17 A 91 / B 87,
  Z 11, 5, 0.5 A 107 / B 80. Of A's 107 at Z 11, 82 become keep-down
  links, 6 plunges, and 19 leave in steps 1 and 2 (A has 88 spans at
  Z 11; B has 71 of its 80). The 2D count weighs each re-entry alike; the
  3D stage turns every one within 48 mm into a link.

### Option (b), a 2D score with the 3D link rule: not exact

The best 2D rule is: simulate steps 1 and 2 on the cleaned 2D plan, then
count the surviving re-entries whose jump from the previous kept position
is over `stay_down_mm`, plus 1 for the piece's first entry. It cannot be
exact:

- Step 3 (the lift's air-run entries) and the guards of step 5 (`link_z`
  against `safe_z` and the cutter length, the proof's `from` match) read
  the 3D stock and the mesh, which the 2D producer does not have.
- `min_region_cut_length_mm` and `stay_down_mm` are 3D parameters
  (`Adaptive3dParams`); `geodesic_segments` gets `AdaptiveParams` only.
  The rule would need them threaded into the producer.
- On D17 it would pick right (the 3D-only plunges are the same in both
  arms), but that is this fixture, not a rule. And I cannot compute the
  2D jumps of D17 without a planner run, so "exact on D17" is not
  measured either; only the 3D side is.

### Option (a), the 3D score: exact by construction — RECOMMENDED

Score each variant by the entries the 3D stage itself makes, on scratch
state.

- **Where:** `clear_one_region` (`adaptive3d/clearing.rs`, C22's file).
  The region body after the 2D plan (the cleanup, F-038, the turn split,
  the lift, the sink with its stamping, the held links) becomes one
  function over the 2D plan and a `LevelEmission`. Under
  `ArcPick::Replay` the region:
  1. gets both 2D plans (a new C12-block door, `slice_variants`, that
     returns plan A and plan B as `SlicePlan`s, the pick off for each:
     `GeodesicOptions::pick_variants`);
  2. runs the region body for A on a scratch copy of the level state (the
     planner stock `TriDexelStock`, the cursor, the level's segment list,
     the planner engagement list, the level metrics and counters, the
     research turn records), and the same for B;
  3. scores each scratch result: the entries (`Rapid`, `RapidWithFloor`)
     the region added that the emitter will NOT turn into a keep-down
     link. The test is the emitter's own: `try_emit_stay_down_link` is
     split into a pure `stay_down_link_z(from, to, descend, ceiling, mesh,
     index, cutter, stay_down_mm, clearance, leave, safe_z) -> Option<f64>`
     and the emission. The scorer calls the pure part with the proof's
     `from`, the draped entry (`drape_point`) and `descent_start_z`, as the
     emitter does. The emitter calls the same function, so the two cannot
     drift (shipped bytes unchanged: the same arithmetic, moved).
  4. keeps the variant with fewer plunges; on a tie the one with less
     keep-down length (held links plus stay-down links, XY); on a tie of
     both, A. It moves the winner's scratch state into the real level
     state, so nothing is planned a third time.
- **The one approximation:** the emitter also needs its previous emitted
  position within `STAY_DOWN_FROM_XY_EPS_MM` (0.05 mm) of the proof's
  `from`. The scorer assumes it (the drape and the RDP keep a cut's end
  point, `path.rs` says so). The `info` line prints the scored plunges per
  region, so the run checks it against the trace's plunge spans per level.
- **No new constant:** every threshold is the emitter's own value.
- **The 2D producer:** `geodesic_segments` keeps the C28 2D pick for the 2D
  harness. I agree that the 2D harness stays on the 2D score: the 2D
  harness has no 3D stage, and its "entries (plunges)" are exactly the 2D
  re-entries (D17: star 38 = 38, pocket 44 / 26 as the 2D runs read). In
  the 3D arm, `ArcPick::Replay` is read by `clear_one_region`, and the
  slice door asks for the plans with the pick off, so the 2D pick does not
  run there.

**Cost in planning time.** Per region of the research arm under the pick:
two 2D plans and two cleanups (as C28, less the third cleanup), two runs
of the 3D region body (lift, stamping, held links) on scratch state, one
stock copy per variant, and one mesh probe (10 drop-cutter samples) per
entry within 48 mm. So about 2 × the region's planning time of one
variant, plus the stock copies. On Corne the zigzag arm plans in about
10 s (D-FIX2), so the pick takes about 20 to 25 s. C28's pick (three
cleanups, no 3D body twice) was not timed at D17. The stock copy is the planner `TriDexelStock` of the
op; its size is the op's grid, copied twice per region.

### Predictions (NOT results)

| Case | Kept (3D score) | Plunges | Cycle |
|---|---|---|---|
| Corne "3D Rough 1", each level | A (A 10/6/6/34 against B 14/8/8/37) | 56 (= zigzag) | 2607 s (D17 zigzag; v2 and the C28 pick: 2815 s) |
| C12 plain test (one piece) | B (D16a: 48 against 68 entries) | 48 | — |
| 2D harness | unchanged (C28's 2D score) | star 38, pocket 26 | — |

The scored plunges per region (`info`) must equal the trace's plunge spans
of that region's level minus the level's shared ones (the first entry; on
Corne Z 0.50 the 23 small-region plunges).

### Who edits what

- C12 block of `research_geodesic.rs` (C22's): `slice_variants`.
- `adaptive3d/clearing.rs` (C22's): the region body split, the scratch
  runs, the score and the commit.
- `adaptive3d/path.rs` (shipped emitter): `stay_down_link_z` split out of
  `try_emit_stay_down_link`, the same arithmetic (byte parity holds).
- Mine: nothing in `research_geodesic.rs` outside the C12 block except the
  doc of `ArcPick::Replay` (the 3D arm scores in 3D).

The lead's ruling decides who builds it; the clearing.rs and path.rs
parts touch C22's work and the shipped emitter.
