# P15 A3: the coverage target is the plan's reachable floor

- Editor: A3, 2026-10-23, worktree `adaptive-head`, branch
  `adaptive-instruments` (head 26355fe9). No cargo was run; no commit.
- Spec: STATUS.md "Lead ruling 19:30, amended 19:40 ... on the coverage
  target (A3)". Evidence: P15_E_evidence.md §E2, DEFECTS.md G-WALLSTRIP,
  G-WALLSTRIP-PLAN.

## 1. The per-centre intended floor (from the planner's own code)

I read `path.rs` ("Compute Z levels", `step_levels`, the level loop, the
drape), `clearing.rs` (`cell_has_material`, `level_gate`, the
ContourParallel rings and clean-up raster, the AgentSearch lift) and
`search.rs`. The facts:

1. The planner's surface is `SurfaceHeightmap::from_mesh(mesh, index,
   cutter, …, mesh.bbox.min.z)`: the drop-cutter TIP Z of the op's cutter,
   floored at the mesh-bbox floor where the cutter meets nothing (through
   holes, outside the part). It is not the model Z under the centre.
2. A level at `z_level` cuts a centre to `(surf + stock_to_leave).max(z_level)`.
   Four sites held a copy of this expression: `cell_has_material` (the
   material test of every strategy), the AgentSearch lift
   (`clear_one_region`), the ContourParallel clean-up `lower`, and the
   debug diagnostic in `search.rs`. The ContourParallel rings use the same
   value at blend 0 (`(z_level + 0·…).max(target_z)`).
3. The emitter's drape lifts a point to `point_drop_cutter + leave`: the
   same surface function, so at a grid centre it adds nothing.
4. The last level is `(surface_bottom + leave).max(pinned bottom_z)`. Every
   level drapes, so over the whole plan a centre's floor is the last
   level's floor:

   ```text
   floor(c) = max(dc(c) + leave, z_bottom),  z_bottom = max(min dc + leave, pinned)
   ```

**No steep-slope guard holds a level as intent.** The lead's model
("the level Z where the guard holds the level") has no counterpart in this
planner. The one hold is the F-029 bite clamp of the ContourParallel
clean-up raster (`stock_top − depth_per_pass − tolerance`). It reads the
current stock, so it limits one pass; it is not a floor the plan intends.
So a Z-level terrace above the draped floor is NOT the plan's floor, and
it still reads as an island. Spec item 1 said "if it cannot be factored,
terraces stay flagged and the note says why": it CAN be factored, and the
planner's own rule says a terrace is not intent.

### The shared function (one source)

New file `crates/rs_cam_core/src/adaptive3d/intended_floor.rs`:

- `level_floor_z(surface_z, stock_to_leave, z_level)` =
  `(surface_z + stock_to_leave).max(z_level)`. The four planner sites now
  call it (`clearing.rs` ×3, `search.rs` ×1).
- `plan_bottom_z(surface_bottom, stock_to_leave, pinned)` =
  `(surface_bottom + stock_to_leave).max(pinned.unwrap_or(NEG_INFINITY))`.
  `path.rs` "Compute Z levels" now calls it.
- `intended_floor_grid(mesh, index, cutter, FloorGrid, stock_to_leave,
  pinned)` (pub, re-exported from `adaptive3d`): builds the planner's
  heightmap with the same constructor and floor on the caller's grid and
  maps `level_floor_z(s, leave, plan_bottom_z(min s, leave, pinned))`.

Behaviour-neutral: each call site has the same float operations in the
same order as before (diff: `clearing.rs` 3 lines, `path.rs` 1 statement,
`search.rs` 1 line). No copy of the rule remains in `stock/`.

Limit: the measure samples the drop-cutter function on the checkpoint grid,
not on the planner's grid (cell `max(R / 6, tolerance)`, one radius past
the mesh bbox). `min dc` is the mesh-bbox floor on both grids whenever each
grid holds a centre farther than R from the model.

## 2. The reachable envelope (the erosion)

```text
target(x) = min over centres c with |x − c| ≤ R of floor(c) + h(|x − c|)
```

- Centres: the cell centres of the stock grid; with the boundary on, only
  those inside `boundary_centre` (`centre_mask`, unchanged).
- `h(r)` = `MillingCutter::height_at_radius(r)`, `R` = `cutter.radius()`.
  The flat cutter has `h = 0`: the target is the erosion of the floor by
  the disc (the morphological opening of the model by the tool, plus the
  leave). A ball or bull nose adds its profile height exactly; this is not
  an approximation, only a slower path.
- A column with no centre within R is outside the population (`NAN`).

Cost, with `k = ⌊R / cell⌋`:

| Step | Cost |
|---|---|
| intended floor | one drop-cutter query per cell (the planner's own heightmap cost at the checkpoint cell), rayon-parallel |
| erosion, flat cutter | per output row, one monotone-queue sliding minimum per disc row: `cells × (2k + 1)` window steps, each amortised O(1); rows in parallel |
| erosion, other profiles | bounded window: `cells × disc cells` ≈ `cells × π k²` |
| wall mask | `cells × 8` |

Corne at 0.3 mm: 6 mm tool k = 10 → 21 steps per cell; 3 mm tool k = 5 →
11. wanaka at 0.5 mm, 6 mm tool: k = 6 → 13. The heightmap build dominates;
E3 must time the adopt (E2: 2.0–4.7 s).

Limits:

- The centres are grid points, a subset of the centres the tool can take.
  The minimum over a subset is never below the true one, so the target can
  stand up to about one cell of slope ABOVE the true reach, never below. A
  sliver of ≤ 1 cell against a wall that is not parallel to an axis can
  therefore read as unreachable (no island).
- Boundary off: centres off the stock grid are not counted, so a column at
  the grid edge can read a higher target than the tool can leave there.
- The erosion takes EVERY grid centre, as the ruling says. The plan visits
  centres only at its stepover, so the stepover scallop of a flat tool on a
  slope (about `stepover × tan θ`) stands above this target and reads.

## 3. Wall strips stay findings

New field `StandingIsland::touches_wall`: a cell of the island is within
one cell (8-neighbour) of a column whose target stands more than one depth
per pass above that cell's target, OR of a column outside the population
whose stock top stands that high. The second case is the spec's rule with
the floor of an unreachable column taken as its stock top (the plan leaves
it whole). No class reads the field. The finding message counts the
islands at a wall and marks each listed one "at a wall"; the narration
sentence counts them.

`RoughTarget` gains `cutter` and `depth_per_pass_mm`; `RoughReach` is
deleted (no legacy) and its polygons move to `RoughTarget::centre`.

## 4. The bar (unchanged)

```text
standing = stock_top − target > stock_to_leave + one cell
```

The target holds the leave once (inside `floor`); the bar adds it once
more, as before. The one cell is the G-OVERCUTREF tolerance
(`commanded_tolerance_mm`). The class rule is unchanged (Safety above one
depth per pass + one cell).

## 5. Where my reading differs from the ruling (for the lead)

1. **Terraces.** The ruling expected the guard's held levels to be intent.
   The planner has no such guard; every level drapes. A terrace reads.
2. **The outer edge fringe.** The ruling expected the erosion to remove
   it ("no reachable centre covers it"). The erosion does not remove it.
   - The A2 population was already conservative: `R − cell` from the
     containment polygon. The fringe columns lie inside it. So a centre
     inside the containment DOES reach them.
   - The planner's outline (the pre-clear) is the raw boundary. The
     containment is the boundary inset by R (Inside).
   - The ContourParallel rings start `min(R, stepover / 2)` inside the
     material edge, which is the outline. The centre clip then drops each
     ring outside the containment.
   - Arithmetic for the Corne 6 mm rough, if its stepover is 2.4 mm (check
     it): the first ring sits 1.2 mm inside the outline and is clipped. The
     next ring sits 3.6 mm inside, 0.6 mm inside the containment. Its cutter
     edge reaches 2.4 mm past the containment. The A2 population reached
     2.7 mm. The difference is 0.3 mm: one cell at 0.3 mm. That is E2's
     one-cell fringe.
   - So the fringe is G-WALLSTRIP-PLAN on the outer edge (E2 suspected the
     same behaviour). It is reachable material the plan left. It still
     reads, now with `touches_wall = true`, because it stands next to the
     uncut stock outside the reach.
   - The hex-side stair-step is a different class. Those ≤ 1-cell slivers
     fall to the grid-subset limit of §2, so most of them go.
3. **Population radius.** The population is now "a grid centre within R",
   not "within R − cell of the polygon". It is exact for the centres it
   counts, so it can be one cell WIDER than A2's.

## 6. Predictions for E3 (four Phase 0 controls)

### Corne ContourParallel (the zero control)

- The hex-corner residue of the 6 mm tool reads NOTHING (E2: 151
  islands, about 22 mm² each).
- Most of the stair-step slivers around the hex sides go.
- The wall strips still read. They are 0.6–1.4 mm wide and 14.5 mm tall
  (E2: 80.7, 61.4 and 24.8 mm²). `touches_wall` is true. The class stays
  Safety, because they are taller than one step.
- The outer-edge fringe still reads, with `touches_wall` true.
- The 5 small round holes read nothing on op 0 if they are narrower than
  6 mm. On op 1 (3 mm) a core reads only where the hole is wider than 3 mm.
- So the zero control still FAILS. It fails honestly, on the strips and the
  fringe (G-WALLSTRIP-PLAN).

### Corne AgentSearch

- The 37 hex cores read: 1.4–1.86 mm wide, 3.5 mm tall, `touches_wall`
  false. Reason: in a hex about 9 mm across, the floor-to-plate step of the
  target is about R + (9/2 − R) ≈ 4.5 mm from the hole centre. A core is
  less than 1 mm from the centre. So no core cell is within one cell of the
  step. E3 falsifies this if a core reads `touches_wall` true.
- The band reads, with `touches_wall` true (it stands against the wall).
- The spur reads if it stands free: 13 mm × 0.3 mm near (102.9, −22.3),
  `touches_wall` false.
- The wall strips read, with `touches_wall` true.
- The ~1170 one-cell islands fall to a fraction: the outer-edge fringe only.

### Corne ContourSpiral

- 2 cores at (19.6, −84.9) and (54.3, −85.0), `touches_wall` false.
- The band, with `touches_wall` true. No spur.
- The 5 small-hole residues read only where the 3 mm tool fits the hole.

### wanaka200

- Back rough: abstains `setup_not_identity` (face up Bottom), unchanged.
- Front rough: the slope shadow of the flat tool and the valley-floor
  wedges read nothing. These were part of E2's 830 islands of 1–2 mm.
- Front rough: a held terrace still reads if it is material above the
  draped floor. The stepover scallop reads on slopes steeper than about
  `atan(bar / stepover)`.
- Front rough: the 9.31 mm boundary frame reads with `touches_wall` true.
- The lead's "no terraces" does NOT follow from the planner's rule. E3
  tells which class remains.

## 7. Files

- `crates/rs_cam_core/src/adaptive3d/intended_floor.rs` (new)
- `crates/rs_cam_core/src/adaptive3d/{mod.rs, clearing.rs, path.rs, search.rs}`:
  the calls to the shared functions; `mod.rs` re-exports `FloorGrid`,
  `intended_floor_grid`.
- `crates/rs_cam_core/src/stock/standing_islands.rs`: `RoughTarget`, the
  erosion (`disc_rows`, `reachable_floor`, `sliding_min`), `wall_cells`,
  `touches_wall`, the doc, the narration; unit pins
  `the_flat_erosion_is_the_minimum_over_the_disc`,
  `the_ball_erosion_adds_the_profile` (both against a brute-force disc
  minimum).
- `crates/rs_cam_core/src/stock/sim_triage.rs`: the message, the test fixture.
- `crates/rs_cam_core/src/session/compute/simulation.rs`: passes
  `&inputs.tool_def`, `cfg.depth_per_pass`, the centre polygons.
- `crates/rs_cam_core/src/diagnostics/ids.rs`: doc of
  `PROJECT_ROUGH_LEAVES_ISLANDS` only (it named the A2 target).
- `crates/rs_cam_core/tests/a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs`:
  arms (a)–(d) on the new target, plus (e) valley slope + held level, (f)
  fringe outside the reach / strip inside it, (g) wall strip, (h) hole core
  with corner residue. The expected stocks are closed-form openings of the
  model by the 6 mm flat disc, not outputs of the measure.
- `crates/rs_cam_core/src/stock/CLAUDE.md`, `crates/rs_cam_core/src/adaptive3d/CLAUDE.md` (both 40 lines).

`rustfmt --check` (edition 2024, children skipped) is clean on every Rust
file I touched.

## 8. Verifier commands (through `scripts/cargo_lane.sh`, each under `timeout`)

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core -q --lib standing_islands
cargo test -p rs_cam_core -q --lib sim_triage
cargo test -p rs_cam_core -q --lib adaptive3d
cargo test -p rs_cam_core -q --test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov
cargo test -p rs_cam_core -q --features test-support --test adaptive3d_emission_byte_parity --test adaptive3d_plan_matches_merged_path_g_plansimgap --test adaptive3d_planner_never_ahead_of_emitted_path --test adaptive3d_global_gate_drapes_below_floor
cargo test -p rs_cam_viz -q --lib g_roughcov
cargo test -p rs_cam_viz -q --lib
cargo test -p rs_cam_mcp -q
cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
```

Risks to watch in the run (checked by reading, not by cargo):

- Arm (e): the held band has a 0.05 mm margin at x = 28.0. The target
  there comes from the grid centre c = 25.0, at exactly R. Its disc rim
  touches the plane at x = 28.0, so the drop-cutter must be exact to
  0.05 mm. If the arm fails there, assert the column count as a range. Do
  not move the band.
- The viz sentry `g_roughcov` keeps one 9 × 17 island only if the sample
  tool's centres reach the block. They did at `R − cell`, so they do at R.

# A4 (2026-10-23, after E3): free islands first, an op-independent wall tag

Source: P15_E_evidence.md §E3 (main checkout). E3 read every eye defect
on the A3 target: 37 cores, 2 cores, the spur (13.0 × 0.3 mm) and the
band. The terraces and the screw holes are gone, and the wanaka front
fell from 910 to 178 islands. Two defects remained.

## 1. The headline named only wall islands

The band or the fringe headed "tallest first" on every rough, so the free
cores and the spur never reached the headline.

Fix: `measure_rough_coverage` sorts the islands free first
(`touches_wall` false), then the ones at a wall. Within each group the
order is tallest first. The finding's list follows that order. The message
now says "Free islands first, then the ones at a wall, tallest first in
each group". The narration sentence is:

```text
coverage: N free island(s) stand more than B mm above the target (…); the tallest is H mm high and W mm wide at (x, y); M more touch a wall; A mm2 in all.
coverage: no free island stands more than B mm above the target (…); M island(s) touch a wall, the tallest H mm high and W mm wide at (x, y); A mm2 in all.
```

The class rule is unchanged: it counts every island over one step + one
cell, whatever the order.

## 2. The wall tag read the op, not the wall

A3 tagged a cell when a neighbour's target stood more than ONE DEPTH PER
PASS higher. So a 3.5 mm hex rim was a wall for the 3 mm op (step 3) and
not for the 6 mm op (step 6): E3 counted 627 untagged rim slivers on op 0.

New rule (in `StandingIsland::touches_wall`):

```text
rise(i)      = max over the 8 neighbours j of cell i of  wall_top(j) − target(i)
wall_top(j)  = target(j), or the stock top of j when j is outside the reach
touches_wall = max over the island's cells of rise(i) > max_standing − one cell
```

Derivation:

- Material that leans on something at least as tall as itself is a strip
  against a wall. Material with nothing that tall within one cell stands
  free (a hole core, a spur).
- The scale is the island's own peak height, `max_standing`. The rule
  reads no op parameter, so two ops tag the same sliver the same way.
  `RoughTarget::depth_per_pass_mm` is deleted, so the compiler holds this.
- A strip left at the wall top stands EXACTLY as high as the wall rises. A
  strict "rise > max_standing" misses it. So the test allows one cell: the
  G-OVERCUTREF tolerance, the same quantisation of a column top that the
  bar allows.
- The plan leaves an unreachable column whole, so its stock top is its
  wall top (the outer fringe and the band against the uncut stock).

**The diagonal neighbours.** The A3 search already covered the 8 cells
round each cell (`dr, dc ∈ {−1, 0, 1}`). So the 41 free one-cell slivers
at the foot of the op 1 angled walls (0.8–1.1 mm tall) did not come from
a 4-neighbour search. I think they come from the threshold: the target
there rises by less than op 1's 3 mm step but by more than the sliver.
The new rule compares the rise with the sliver's own 0.8–1.1 mm, so they
should tag. E4 confirms. If they still read free, the high column is two
cells away, and the cause is the grid-subset limit of §2. Then a wider
ring is a new decision for the lead.

A steep slope also tags. Its rise per cell, `cell × tan θ`, exceeds a
short island's height minus one cell when θ is steep enough. That is a
"wall" in this rule's sense; the class does not change.

## 3. Sentry arm (i)

`the_wall_tag_reads_the_island_and_free_islands_come_first`: the plate
with the 10 mm hole of arm (h).

- A one-cell sliver at the hole rim stands 4 mm, as high as the wall
  rises. The 6 mm cutter and the 3 mm cutter both tag it.
- A core in the hole centre stands 1.5 mm. It is SHORTER than the sliver,
  but it stays free and comes first in the list.
- The narration starts "coverage: 1 free island(s)" and says "1 more
  touch a wall".

Arms (a), (d), (e), (h) keep `touches_wall` false; arms (f), (g) keep it
true under the new rule (rise 7.5 vs 7.5 and 4.0 vs 4.0, inside one cell).

## 4. The adopt cost (measured by E3, not optimised)

E3 on wanaka200: the adopt took 18.5 s, against 4.7 s in E2. Only the
front rough measures; the back rough abstains before the target is built.
With `N` = the cells of the checkpoint grid and `k = ⌊R / cell⌋`:

```text
cost ≈ N drop-cutter queries            (intended_floor_grid: the planner's heightmap on the checkpoint grid)
     + N × (2k + 1) sliding-window steps (the flat erosion)
     + 8 N                               (the wall rise)
```

At the 0.5 mm wanaka cell and the 6 mm tool, k = 6, so the erosion is 13
steps per cell. The drop-cutter queries are the larger term: each query
searches the mesh under a 6 mm disc. A later pass could reuse the
planner's own heightmap (same function, its grid) or sample it coarser and
interpolate. That is not done now (coordinator ruling).

## 5. Files (A4)

- `crates/rs_cam_core/src/stock/standing_islands.rs`: `wall_rise`
  (replaces `wall_cells`), the per-island rule in `group_islands`, the
  free-first sort, the narration, the field doc; `depth_per_pass_mm`
  deleted from `RoughTarget`.
- `crates/rs_cam_core/src/stock/sim_triage.rs`: the message wording and doc.
- `crates/rs_cam_core/src/session/compute/simulation.rs`: no depth per pass.
- `crates/rs_cam_core/tests/a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs`:
  arm (i), the shared hole fixture (`plate_with_hole`, `hole_target`).
- `crates/rs_cam_core/src/stock/CLAUDE.md` (40 lines).

`rustfmt --check` is clean on all four Rust files. No cargo was run.
The verifier commands of §8 above cover A4 too.
