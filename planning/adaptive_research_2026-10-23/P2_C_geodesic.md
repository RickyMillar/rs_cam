# P2 editor C — candidate (a), the geodesic wavefront (2026-10-23)

Status: code and tests written. **No cargo was run** (the brief says so). A
verifier must build, lint and run the tests below. Nothing is committed.
Worktree: `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`,
base HEAD a23df1f8.

## What I built

1. `crates/rs_cam_core/src/adaptive/research_geodesic.rs` (new, behind
   `#[cfg(feature = "research")]`, declared in `adaptive/mod.rs`).
   - `GeodesicField`: the geodesic distance T from the entry disc over the
     tool-centre region. Holes and the outside are unreachable (INFINITY,
     never updated). T is in stepovers.
   - The producer (`geodesic_segments`): the shipped starter pocket, then the
     level sets T = 1, 2, ... stepovers as `Cut`s and `Link`s.
   - The pipeline after the producer is the product one: the cleanup
     dispatch of `mod.rs` (`apply_contour_parallel_residue_cleanup` for the
     default `ContourParallelHybrid`), which replays every cut and link under
     the pass load, then the residue contours, the wall pass
     (`finish_walls_within_pass_load`) as the final pass, and the mop. Then
     `segments_to_toolpath`.
   - `PassLoadReadout`: the planner's own pass-load reading of a plan,
     replayed on a fresh grid (`measure_step`, `step_within_pass_load`).
   - `run_2d(outline, params, Candidate2d)`: the harness door. It plans each
     inset piece (`inset_piece_outlines`) with a shipped strategy (product
     pipeline) or with candidate (a), and returns the toolpath, a summary,
     the field reports and the wraps.
2. `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs` (new,
   `#[ignore = "instrument: .."]`, `[[test]]` with
   `required-features = ["research", "test-support"]` in
   `crates/rs_cam_core/Cargo.toml`).
3. Unit tests in the module (three, listed below).
4. Docs: `src/adaptive/CLAUDE.md` (a Research line in Files and in
   Sentries; 39 lines), `tests/CLAUDE.md` (`research` 9 → 10).

## Reuse and the edits to shipped files

Every edit to a shipped file is a visibility change or a behaviour-neutral
factor. No shipped behaviour changes.

| File | Change | Why |
|---|---|---|
| `finish/scallop_isofield.rs` | `fast_sweep` → `pub(crate)`, returns `bool` (converged); `build_field` ignores it (`let _converged`) | THE one Eikonal solver, reused as is (no copy). The flag feeds `FieldReport::converged` |
| `finish/scallop_isofield.rs` | `iso_contour` body moved to `pub(crate) fn iso_contour_where(field, level, cell_kept)`; `iso_contour` calls it with a filter that keeps every cell | The candidate drops each cell with an unreachable corner (see "Contour"). No copy of the grid walk |
| `adaptive/path.rs` | `emit_helical_starter_pocket`, `subdivide`, `LinkLoad::stamp_entry`, `LinkLoad::plunge_departure_exempt` → `pub(super)` | The starter pocket and the entry/readout rules of the replay |
| `adaptive/spiral.rs` | `nearest_index` → `pub(super)` | The spiral's seam rule for a closed wrap |
| `adaptive/mod.rs` | `#[cfg(feature = "research")] pub mod research_geodesic;` | The declaration |

Reused unchanged: `adaptive_segments_with_debug`, `apply_*_cleanup`,
`segments_to_toolpath`, `LinkLoad::new`, `pass_load_for`, `measure_step`,
`step_within_pass_load`, `ToolCentreRegion`, `MaterialGrid`,
`inset_piece_outlines`, `geometry::marching_squares` (`cell_case`,
`cell_segments`, `chain_segments`, `CHAIN_EPS`), `geo::point_to_segment_distance`.

Added (no equivalent in the repo): the disc seed; the reachable mask; the
join of open-arc fragments (`chain_segments` extends a chain at its tail
only, so an open arc can come back in pieces); the wrap order; the
pass-load readout; the run door.

One copy: `run_2d`'s `cleaned` repeats the 8-line cleanup `match` of
`adaptive_toolpath_structured_annotated_traced_with_cancel` (`mod.rs`). The
harness asserts that each shipped column equals `adaptive_toolpath` move by
move, so a drift of that copy goes red.

## Derived constants

| Constant | Value | Derivation |
|---|---|---|
| Field cell | `max(R / 6, tolerance)` (0.5 mm at R 3, tol 0.1) | The planner cell, `path.rs:699` `let cell_size = (tool_radius / 6.0).max(tolerance);`. The field is solved on the planner's own material-grid lattice |
| Seed disc radius | `R` | The starter pocket ends with one lap of the cutter centre at `r = R` (`path.rs` `starter_spiral_points`, called by `emit_spiral_starter_pocket`). That lap is the cleared rim the front starts from. The helix radius only sizes the plunge hole stamp |
| Seed centre | the plunge point of the starter | `find_entry_via_distance_transform` (EDT max), read from the `StarterPocket` marker |
| Seeded nodes | every reachable node within one cell of the disc, fixed at `(d − R)/s` | The way `seed_boundary` pins the region edge (exact within one cell) |
| Speed | the op stepover at every reachable node | T is in stepovers, so level k is k stepovers out |
| Wrap spacing | one stepover; wrap k at T = k, k ≥ 1 | The contour spiral's rule, `spiral.rs:128` `first_offset` (the starter rim), `:134` `off += stepover`. Wrap 1 here is the spiral's first biting wrap |
| `reach` | `s + (1 + √2) · cell` | One level spacing, plus the field error (about one cell, the disc test), plus one cell diagonal (the longest contour segment in a cell) |
| Closed chain | ends ≤ `√2 · cell` apart | `chain_segments` drops a loop's closing point: the gap is one contour segment |
| Fragment join | ends within `CHAIN_EPS` | The chaining tolerance of `geometry::marching_squares` |
| Readout step | `3 · cell` | The planner pass step, `path.rs:133` `step_len: grid.cell_size * 3.0` |

Inherited and not derived (named, not mine): `fast_sweep`'s `MAX_ROUNDS = 24`
and `TOL = 1e-6` (iso-scallop module). The harness sets
`trochoid_cap_mult: 1.2` as `clearing_2d.rs` sets it for 2D Adaptive; under
the 2D pass load the spiral does not read it.

## Design decisions

### Contour: a wrap ends at a wall

`iso_contour` maps an unreachable node to `f64::MAX`. The interpolated
crossing then lands ON the reachable node, and every level set past first
contact runs along the wall or the island lattice. That is a contouring
artefact, not a property of the field. The candidate contours only the cells
whose four corners are reachable (`iso_contour_where`). So a wrap is the
front only: a closed loop while the front has not reached a wall, an open
arc after. The arc ends within about one cell of the wall; that strip is the
wall pass's work. I chose the interpolated contour of L7 over the boolean
marching squares of L3 because T is a scalar field: the interpolation places
each crossing to the field's precision, and the boolean form adds a
half-cell placement error.

### Join

- A closed wrap: the seam rule of `spiral.rs` (`nearest_index`), lapped and
  closed on its seam.
- An open arc: from the end nearer the cutter. In a lobe the arcs alternate
  direction (climb, then conventional). Say so on the page.
- A join no longer than `reach` is a cutting step inside the `Cut`, as the
  spiral's seam step is. A longer join is a keep-down `Link`; the replay
  admits it under the pass load or turns it into a retract and re-entry.

### Order (the rule the brief asks for)

A wrap at level k is cut only after every wrap at level k − 1 within `reach`
of it. Among the wraps that may be cut, the one nearest the cutter is next.
So each wrap has the previous level, one stepover away, on its cleared side,
and the cutter fills one lobe to its end before it moves to the next. A
level-by-level order would also hold the cleared side, but it jumps between
all lobes at every level.

### Over-load steps: no trochoid insert

The contour spiral's trochoid insert carries tuned values with no derivation
(loop radius `max(s, 0.4 R)`, pitch `0.6 s`, 16 directions, fill 0.8, 20
points). Under the operator ruling of 2026-10-23 the candidate does not take
them. A step over the ceiling takes the replay split (`emit_capped_walk_with`),
the fallback every 2D producer has. STATUS ruling 3 already owns the
removal of the spiral's cap.

## Tests

Unit tests in `research_geodesic.rs` (`--features research`):

- `the_field_on_a_disc_is_the_euclidean_distance_within_one_cell`: a 20 mm
  disc, two seeds; T never under-reads the Euclidean distance and over-reads
  it by at most one cell. Prototype (Python copy of the sweep): 0.62 and 0.72
  cells.
- `the_field_goes_round_an_island`: 60 × 40 mm with a 6 mm round island;
  behind the island T > Euclidean + one cell, and T ≥ exact geodesic − one
  cell. Prototype: T 34.6 mm, Euclidean 31.0, exact 33.36.
- `every_emitted_step_holds_the_pass_load`: candidate (a) on a 60 × 40 mm
  pocket with a 6 mm island, full pipeline; the readout asks
  `step_within_pass_load` of every step; 0 over. Not run: this is the claim
  most likely to show a finding (see "Unfinished").

Verifier commands (through `scripts/cargo_lane.sh`, each with `timeout`):

```text
cargo test -p rs_cam_core --features research -q --lib adaptive::research_geodesic
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
cargo test -p rs_cam_core -q --lib adaptive        # shipped 2D unchanged
cargo test -p rs_cam_core -q --lib finish::scallop_isofield
cargo test -p rs_cam_core -q --test scallop_iso_field_config \
    --test folder_maps_name_real_files_f140 --test every_ignore_states_its_kind_f152 \
    --test gate_tests_read_no_operator_paths_f155
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings     # research off
```

## Render outputs

`$ADAPTIVE_GEODESIC_SVG_DIR`, else
`target/tmp/adaptive_geodesic_2d_g1/` (the test's `CARGO_TARGET_TMPDIR`):

- `star_agent.svg`, `star_contour_spiral.svg`, `star_geodesic_a.svg`
- `pocket_agent.svg`, `pocket_contour_spiral.svg`, `pocket_geodesic_a.svg`
- a `.txt` beside each SVG, and `summary.txt` for all six

Each SVG: cuts blue, keep-down links orange, rapids dashed grey, entries red
rings, the outline black, candidate (a)'s wraps pale grey. The caption and
the `.txt` give the pieces, the cut and link length, the entries, the lifts,
the keep-down links, the pass-load readout and (for (a)) the field report
(seed, levels, wrap pieces, converged). No rank and no score.

Columns: Agent, ContourSpiral, candidate (a). The 2D op exposes no
contour-parallel strategy (its contour-parallel path runs only behind the
narrow gate), so there is no contour-parallel column.

Params: `AdaptiveConfig::default()` (stepover 2.0, tolerance 0.1,
`ContourParallelHybrid`), the fixture's 6 mm end mill, and the helix radius
of a new Adaptive toolpath (`DressupConfig::for_op(OperationType::Adaptive)`
→ `helix_radius_for(..).emitted_mm`, 0.3 · D = 1.8 mm).

## Known risks (R3, Source 3)

- Rarefaction at the source: held by the exact seed within one cell.
- Rarefaction at an island's convex corner: the first-order update
  over-reads the distance (2.5 cells behind a 6 mm island). The wraps bunch
  behind an island: safe for the load, not exact for the spacing.
- Shocks where two fronts meet: each wrap that crosses a meeting line has a
  kink; the replay splits a step there that is over the ceiling.
- The 24-round sweep cap: not reached on the prototype shapes (2 to 3
  rounds). A many-island slice may reach it; `FieldReport::converged` shows
  it.
- The field error grows with distance from the seed (0.89 to 1.03 cells at
  45 to 60 mm). It is always an over-read.
- The seam step of a closed wrap is radial; under the swept-width measure a
  radial step into a stepover band reads about 0.94 of D, so the replay will
  refuse it and re-enter. The contour spiral has the same seam. This affects
  only the first wraps (closed loops round the starter).

## Unfinished and hand-offs

1. Nothing ran. The verifier runs the commands above; any red is a finding
   for the lead, not a reason to loosen a test.
2. A closed-wrap seam without a re-entry: contour `T/s − φ/2π` (φ the polar
   angle about the seed) while the wraps are closed loops. That gives one
   spiral with a constant stepover and no radial seam. Not built.
3. The pass-load readout is the planner predicate on the planner grid, not
   the simulation oracle of `a_clearing_cut_holds_the_pass_load`. The fixture
   pages of Phase 2 step 3 need the simulation.
4. FIN-01 (`the_research_arms_are_feature_gated_fin01`) guards only the three
   `finish/` arms. `adaptive::research_geodesic` is outside its two arms. I
   did not edit the sentry; the lead decides whether FIN-01 grows to cover
   `adaptive/`.
5. If `every_emitted_step_holds_the_pass_load` goes red, look first at
   where the over-steps are. If they sit in wall-pass or mop spans, the
   cause can be shipped code, not the candidate: a `cut_only` walk
   (`emit_capped_walk_with`) drops the air steps it already stamped with
   `clear_segment`. A fringe sliver under such a step is then cut on the
   planner grid but by no emitted move, and the readout (which replays only
   emitted moves) reads a later step over `sliver_ceiling`. That is the
   folder rule "Do not stamp stock no emitted move cuts". Report it as a
   shipped-code finding, and do not loosen the test.
6. The harness row carries `"test-support"` as the brief asks. It binds no
   test door. `the_test_doors_are_gated_fld0405` does not parse the
   `[[test]]` rows (checked), so the row does not affect it, and the
   `tests/CLAUDE.md` count of 66 door binaries stays as it is.
7. The SVGs are planner output (no dressup, no simulation). The look pages
   of PLAN.md (rapids-only view, final stock) need the GUI/MCP path, which
   cannot select candidate (a).
