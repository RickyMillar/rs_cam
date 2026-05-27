# Adaptive corner-clearing investigation — 2026-05-28

## Trigger

Operator observation while running `adaptive3d` AgentSearch on the
machine: pass 1 lays down beautiful constant-engagement arcs across the
bulk of a pocket, but as the spiral converges toward narrow regions
(corners, residue strips along the inset boundary) the arc-to-arc
transitions disappear. The tool retracts and re-enters repeatedly,
producing many short cut groups separated by long rapids.

Reference for desired behaviour: BobCAD's offset-pocket clearing
(`https://bobcad.com/wp-content/uploads/2018/08/ADV-Offset-Pocket-Out-300x209.png`)
— concentric morphed spirals that hug the boundary without retracts.

## Approach used to investigate

Two instrumentation tests in `crates/rs_cam_core/src/adaptive/mod.rs`
(`tests` module):

- `instrument_corner_burrow_50mm_square` — runs the planner on a 50mm
  square, dumps cut/rapid/link counts, exit-reason histogram, per-pass
  step/idle/search-eval detail, ForcedClear positions.
- `render_shape_matrix_svg` — runs baseline + offset-pocket-mop
  post-process on five shapes, writes a side-by-side SVG pair per
  shape to `target/adaptive_shape_<name>_{baseline,proper}.svg`.

Post-process functions (test-module helpers; NOT in planner yet):

- `apply_boundary_extend_fix` — absorbs short Rapids into the preceding
  Cut by feeding straight across. **Failed**: real inter-pass Rapids
  span 9.5-53.8mm (median 40.9mm); the straight feeds cut diagonally
  across already-cleared interior instead of following the residue along
  the boundary. Not the right algorithm.
- `apply_offset_mop_fix` — truncates baseline at end of pass 1, then
  emits concentric inward offsets of the machinable region as cleanup
  loops. **Promising**: matches BobCAD aesthetic on simple shapes, but
  has issues on L-shape and donut topologies (see below).

## Per-shape baseline behaviour

Tool radius 3mm (6mm tool), stepover = R (50%), slot_clearing off.

| Shape | Cuts | Rapids | Links | Notes |
|---|---|---|---|---|
| square 50mm | 16 | 15 | 3 | Pass 1 = 325 steps (good spiral). Passes 2-17 = 162 steps total, mostly 1-24 steps each. 11/17 entries near a machinable corner. |
| rectangle 60×30 | 17 | 15 | 2 | Similar fragmentation pattern as square. |
| circle 50mm | 19 | 20 | 3 | More Rapids than the square (20 vs 15) — smooth boundary still produces many short cleanup passes. |
| L-shape | 18 | 15 | 3 | Comparable to square; concave inner corner adds residue. |
| square + 15mm hole | 27 | 27 | 4 | **Worst case**. Donut topology halves available engagement frontier. Pass 1 can't sustain a long sweep. |

All 17 passes on the square exited via `'no direction'` (engagement
floor 0.005 blocks further progress into narrowing residue). Zero
`'idle'` exits. Zero `ForcedClear` events.

## Per-shape offset-pocket-mop behaviour

| Shape | Cuts | Rapids | Links | Δ Rapids vs baseline |
|---|---|---|---|---|
| square 50mm | 4 | 2 | 2 | -87% |
| rectangle 60×30 | 2 | 2 | 0 | -87% |
| circle 50mm | 3 | 2 | 1 | -90% |
| L-shape | 5 | 2 | 3 | -87% |
| square + 15mm hole | 6 | 5 | 1 | -81% |

Cleanup loops are concentric inward offsets of the machinable region,
each emitted as one continuous Cut group. Transitions between
consecutive loops are Links when distance < 6R, Rapids otherwise.

## Issues identified

### I1 — L-shape cleanup loops trace through cleared interior

**Symptom**: visible in `adaptive_shape_l_shape_proper.svg`. Each
inward offset of the L-shape's machinable region is a closed contour
that runs around the entire L. Where the contour crosses the bulk of
the L's arms (already cleared by pass 1's spiral), the cutter is
air-cutting at feed rate. Particularly visible on the inner offset
loops where the contour goes through fully-cleared territory.

**Root cause**: the proper-fix prototype emits each offset loop as a
single closed Cut, regardless of whether the loop's individual edges
have material to clear.

**Candidate fix**: walk each offset contour and check at every step
whether the cutter would clear material there. Break the loop into
material-touching arcs; emit each arc as its own Cut, connected by
Links where the gap between arcs is short, Rapids when far. Trades
"closed loops, much air cut" for "open arcs, less air cut but more
emission complexity".

### I2 — Square + hole baseline shows extreme fragmentation (27R / 27C)

**Symptom**: visible in `adaptive_shape_square_with_hole_baseline.svg`.
Massive fan of Rapids; pass 1 doesn't form a coherent spiral, instead
fragmenting into many short sweeps around the donut annulus.

**Root cause**: the donut's annular width (~22mm in the test fixture)
is comparable to a few stepovers. Once pass 1 makes one wrap around the
annulus, the remaining material is a thin strip that gives engagement
fractions below the 0.005 floor — search returns None — pass exits.
Each subsequent entry picks a fresh boundary point and immediately
exits the same way.

**Candidate fix (sketch)**: the engagement floor and target engagement
need to **adapt to local material width**. In a region narrower than
`stepover × N`, the planner should accept lower engagement rather than
exiting. Two paths:

  - a) Detect "narrow strip" via `boundary_distance_at` reads
    consistently below `2R` and switch to a "follow the strip"
    sub-mode with relaxed engagement target.
  - b) Use the MaterialGrid's residue contours as guidance — when
    pass 1 exits, walk the remaining material's medial axis instead of
    returning to entry selection.

This is a separate workstream from offset-pocket-mop. Adaptive's
small-space behaviour is the underlying issue.

### I3 — Square + hole offset-mop has structural Rapids from donut topology

**Symptom**: `adaptive_shape_square_with_hole_proper.svg` has 5 Rapids
(vs 2 on the square). Inherent to donut topology: each inward offset
of the donut returns **two** disconnected contours (the shrinking outer
boundary, the growing hole boundary). Cutter must Rapid between them
at each cleanup level.

**Not strictly a regression** — the cuts ARE separated topologically
and can't be linked without crossing the hole. But 5 Rapids on a small
fixture means real-world parts with multiple holes will scale poorly.

**Candidate fix (sketch)**: order cleanup contours so consecutive
emissions are spatially close — minimize total Rapid distance via a
simple TSP greedy on the contour-entry points. Doesn't eliminate
Rapids but minimizes their length.

### I4 — Initial-direction wiggle in pass 1

**Symptom**: `adaptive_corner_burrow_50mm_wiggle.svg` shows the first
~12 steps of pass 1 stair-stepping at ±15°/±30° angle offsets, with
cutter sweeps heavily overlapping (redundant material removal). By
step 15+ the smoothing buffer settles and the path becomes a smooth
spiral.

**Root cause hypothesis**: pass 1 entry lands at a machinable corner
(`walk_boundary_for_entry` finds high engagement there). Cutter
straddles two walls → engagement is high but asymmetrically
distributed. Initial `prev_angle` = "toward nearest material" points
diagonally into the bulk. First search candidate at that angle gives
imbalanced engagement; alternative candidates at ±15° give similar
engagement; angle oscillates until the smoothing buffer averages it
out three steps later.

**Candidate fix (sketch)**: either (a) penalize boundary-corner
positions in `walk_boundary_for_entry` so the entry lands at a wall
midpoint, or (b) set `prev_angle` to the boundary tangent for the
first 3-5 steps so the cutter walks along the wall before peeling
inward.

## Recommendation — do NOT land offset-pocket-mop yet

The proper-fix prototype gives the BobCAD aesthetic on simple convex
shapes (square, rectangle, circle) but the wasted-trace problem on
L-shape and the structural-Rapid problem on the donut mean it
isn't strictly better in all cases. Landing it as
`CleanupStrategy::OffsetMop` (opt-in, default off) is technically
safe, but defaults won't change without addressing I1 — current
production parts would see *more* air-cut time on L-shapes than they
do today.

Order of operations I'd propose:

1. **I1 — material-aware cleanup loop emission** (this PR). Walk each
   offset contour, break into material-touching arcs. Validate on the
   shape matrix; rapids should drop further on L-shape and donut.
2. **I2 — small-space tuning of the main spiral** (separate workstream,
   bigger). Adaptive needs to gracefully handle narrow strips before
   the cleanup phase ever runs.
3. **I3 — TSP ordering of cleanup contours** (small follow-up after
   I1). Worth doing for multi-hole parts.
4. **I4 — pass-1 wiggle fix** (independent of cleanup). Cheap. Either
   ship it standalone or bundle with I1.
5. **Once I1-I3 are done**, land as `CleanupStrategy::OffsetMop` with
   opt-in default and measure on the real Wanaka project to confirm
   no real-world regressions.

## Artifacts

Baseline + proper SVGs for all 5 shapes:

```
target/adaptive_shape_square_50_{baseline,proper}.svg
target/adaptive_shape_rect_60x30_{baseline,proper}.svg
target/adaptive_shape_circle_50_{baseline,proper}.svg
target/adaptive_shape_l_shape_{baseline,proper}.svg
target/adaptive_shape_square_with_hole_{baseline,proper}.svg
```

Plus the original square-only renders:

```
target/adaptive_corner_burrow_50mm.svg          # baseline
target/adaptive_corner_burrow_50mm_cheap.svg    # rejected straight-line absorption
target/adaptive_corner_burrow_50mm_proper.svg   # offset-pocket-mop on square
target/adaptive_corner_burrow_50mm_wiggle.svg   # pass-1 first-40-steps zoom
```

Reproduce all of the above via:

```
cargo test -p rs_cam_core --lib adaptive::tests::render_shape_matrix -- --nocapture
cargo test -p rs_cam_core --lib adaptive::tests::render_corner_burrow -- --nocapture
cargo test -p rs_cam_core --lib adaptive::tests::instrument_corner_burrow -- --nocapture
```
