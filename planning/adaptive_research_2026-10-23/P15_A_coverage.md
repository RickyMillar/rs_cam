# P1.5 editor A: the 3D Rough coverage instrument (G-ROUGHCOV + G-THINSTAND)

Editor A, 2026-10-23, worktree `.claude/worktrees/adaptive-head`, branch
`adaptive-instruments` (HEAD a23df1f8). No cargo was run. Nothing is
committed. The verifier builds and runs everything below.

## Three brief facts that the code does not support

1. **`metrology` is gated.** `lib.rs` declares `#[cfg(feature = "test-support")] pub mod metrology;`
   (F085). A product path cannot call `audit_stock_vs_model`. The column
   walk and the flood therefore moved to a new non-gated file,
   `stock/standing_islands.rs`. `union_coverage.rs` now calls it (metrology → stock, never the
   other direction).
2. **Off-model columns.** `audit_stock_vs_model` gives `NAN` (off-model, not
   compared) to a column with no model surface. The Corne hole cores
   (G-HOLECORE) are in through holes, and the outer band (G-OUTERBAND) is
   outside the silhouette. A plain reuse would read zero islands on the
   positive falsifier. The rough's target therefore has a floor arm (below).
3. **Two of the three MCP outputs do not read the triage.** Only
   `run_simulation` and `get_diagnostics` publish it. On the lead ruling,
   `get_toolpath_diagnostics` and `narrate_toolpath` now read it. See
   "Path to MCP".

## What was built

- `stock/standing_islands.rs` (new, product path):
  - `StandingField::walk(grid, target_z)`: THE column walk,
    `standing = stock_top − target`. `NAN` means outside the population.
    `NEG_INFINITY` means cut through.
  - `standing_islands(field, above_mm)`: THE 4-connected flood. Each island
    gives cells, area, max standing, peak `[x, y, top_z]`, centroid, bbox,
    and `min_width_mm`.
  - `measure_rough_coverage(stock, RoughTarget) -> RoughCoverageOutcome`
    (`Measured(RoughCoverage)` or `NotMeasurable(RoughCoverageAbstention)`).
  - Unit tests: `the_width_is_the_inscribed_disk_in_cells`,
    `a_block_outside_the_reach_is_not_compared`,
    `a_cell_coarser_than_the_stock_to_leave_abstains`.
- `metrology/union_coverage.rs`: the walk and the hotspot flood call the
  kernel. The arithmetic, the flood order and the area sort are the same,
  so `union_coverage_m1` and the unit test `audit_separates_standing_clean_and_gouged` keep their claims.
- `stock/sim_triage.rs`:
  - `SimulationTriage::coverage: Vec<RoughCoverageReading>` (serde
    default). It holds every reading: measured with its islands, a
    measured zero, or abstained with its reason.
  - `build_with_rest_context` takes a fifth argument, `rough_coverage`.
    `build` passes `&[]`.
  - `rough_coverage_finding` gives one finding per rough with islands.
  - `commanded_tolerance_mm` is now `pub(crate)`.
  - Unit tests: `g_roughcov_the_class_reads_the_commanded_step`,
    `g_roughcov_the_reading_reaches_the_triage`.
- `session/compute/simulation.rs`: `ProjectSession::rough_coverage_readings(evidence)`.
- `session/compute/diagnostics.rs` (wiring, outside the list): the two
  triage assemblies compute the readings and pass them to
  `triage_from_parts`, which passes them to `build_with_rest_context`. The
  calls are `simulation_triage_with_diagnostics` and
  `triage_with_report_from_parts`, which `simulation_triage_with_report`
  and `verdict_and_triage_with_report` call.
- `diagnostics/ids.rs` (outside the list, one row):
  `PROJECT_ROUGH_LEAVES_ISLANDS = "project.rough_leaves_islands"`. The brief
  says that new ids live with `link_cuts_stock`, and `diag_ids!` is the only
  way an id enters `ALL` (F162).
- `stock/mod.rs`: `pub mod standing_islands;`.

## The measure and its derivation

- **Population.** Each enabled `Adaptive3d` op that has a checkpoint in the
  stored run. The stock is the op's checkpoint `mesh_stock`, after the op.
  With the boundary on, a column is in the population when its centre is
  within `R − cell` of the tool-centre polygons. These polygons are
  `ResolvedGenInputs::boundary_centre`, the polygons that the boundary clip
  keeps the tool centre in. `R` is half the named diameter, the same radius
  as the clip. The tool centre stays inside the polygons, and the cutter
  reaches one radius further. The `− cell` term is there because the
  polygon mask is quantised to cell centres. Thus the population never
  holds a column that the tool cannot reach. With the boundary off, every
  column is in the population.
- **Target (the rough's own).** `s = topmost model surface Z` at the column
  centre (`maps::reach_map::surface_z_at`, the function the union audit
  also uses), or `mesh.bbox.min.z` where the mesh has no surface. Then
  `target = max(s + stock_to_leave_axial, pinned bottom_z)`. The source is
  `adaptive3d/path.rs` "Compute Z levels": the planner reads the mesh-bbox
  floor through holes, and a pinned `bottom_z` floors the level plan.
- **Walls.** The planner offsets in Z only (`stock_to_leave_axial` is its
  only leave dial; L2 deleted the radial one). This reading is vertical
  too. On a wall at slope θ, the leave and an island are `× cos θ` thick
  along the normal. Material that the tool shape cannot reach (an inside
  corner narrower than the tool, a fillet under a ball) reads as an
  island. It is material the rough leaves. The doc comment of
  `measure_rough_coverage` says this.
- **Bar.** A cell is above spec when `standing > stock_to_leave + one cell`.
  The one cell is the G-OVERCUTREF tolerance (`commanded_tolerance_mm`, the
  same function). `standing` is already measured from `model +
  stock_to_leave`, so an above-spec cell carries more than twice the stock
  to leave over the model. This is PLAN.md's "no uncut island bigger than
  the stock to leave". There is no other threshold.
- **Islands.** 4-connected. This is the grouping the union audit has
  always used (`union_coverage_m1` asserts `hotspot_count`), so one kernel
  serves both callers. Two islands that touch only at a corner are two
  islands. The islands are sorted tallest first.
- **Width.** `min_width_mm = (2·d − 1) × cell`, where `d` is the largest
  centre-to-centre distance (in cells) from a cell of the island to the
  nearest cell outside it. The distance comes from
  `geometry::grid_field::distance_transform_2d` on a grid padded by one
  outside cell. This is the diameter of the largest inscribed disk.
  Limits: the value is quantised to one cell (an even strip reads one cell
  narrow, and the floor is one cell). It is the thickest part of the
  island, so a thin spur on a wide low patch above the bar reads the width
  of the patch (the same limit as the disk opening in `spurs.py`). Read it
  with the area and the bbox. The G-SPUR question is answered by the height
  and the width of each island. There is no spur threshold.
- **Class.** `Safety` (Critical) when the tallest island stands above
  `commanded depth per pass + one cell`. This is the reference that
  `crosses_standing_material` uses: a later pass meets more than one
  commanded step in one bite, and a thin island of that height is the
  G-SPUR column that can snap. Otherwise the class is `Action` (Caution,
  category Quality). An op with no commanded step is `Action`.
- **Measurability.** `NotMeasurable` when `cell > stock_to_leave` (or the
  cell or the stock to leave is not finite). The bar is then more cell than
  spec. A `NotMeasurable` reading raises no finding, but it is published
  on `triage.coverage` with its reason. Other abstentions:
  - `SetupNotIdentity`: the checkpoint carries a `mesh_frame`. The stock is
    setup-local, and this pass reads only the identity frame (F-024).
  - `NoModel`.
  - `TargetUnresolved { error }`: `resolve_generation_inputs` failed.
- **Which run.** `self.simulation` is used only when its `boundaries` equal
  `evidence.boundaries` (id, start, end). Otherwise the function returns no
  reading, which means not measured, never clean.

## Finding id and the path to MCP

Id: `project.rough_leaves_islands`. Message: the count, the bar, the total
area and the cell, then each island tallest first (height, width, area,
peak XY), up to `ADVISORY_CAP_PER_TOOLPATH`, then "N more". When the class
is safety, the message also gives the count of islands over the step.
Evidence: `GeometryCompare` (the tallest island against the bar or the step
bar). `worst.position` is the island peak.

- `run_simulation`: `notify_mcp_simulation_complete` → `build_mcp_diagnostics`
  (viz `controller/events/compute.rs`) →
  `ProjectSession::simulation_triage_with_diagnostics` →
  `rough_coverage_readings` + `triage_from_parts` →
  `build_with_rest_context` → `resp["triage"]`. The response holds
  `triage.safety` / `triage.actions` and `triage.coverage`.
  In that handler, `Command::AdoptSimulation` is applied (compute.rs:922)
  before `notify_mcp_simulation_complete` (compute.rs:996). Thus
  `self.simulation` is the run being reported, and `same_run` holds (this
  was read from the code, not run).
- `get_diagnostics`: the same `build_mcp_diagnostics`.
- No gate: nothing outside the tests refuses export or sets a project
  verdict from `triage.actions` or `triage.safety`. The CLI
  (`rs_cam_cli/src/project.rs`) only prints them. A safety-class finding
  shows in red and blocks nothing.
- `get_toolpath_diagnostics` (lead ruling 2026-10-23, done):
  `mcp_get_toolpath_diagnostics` → `ProjectSession::diagnose_toolpath_with_evidence`
  (`session/compute/export.rs`) now appends
  `simulation_triage(evidence).own_toolpath_findings(id)`.
  - **Appended:** the safety and action findings of the rules the triage
    computes itself (`sim_triage::TRIAGE_OWN_TOOLPATH_RULES`), with scope
    `Toolpath { id }` equal to this op: `rough_leaves_islands`,
    `link_cuts_stock`, `crosses_standing_material`, `entry_load`,
    `plunge_class_load`, `rest_slower_than_fresh`. Each finding carries the
    toolpath id in its scope. No owner is guessed. No other source makes
    these ids, so none is listed twice.
  - **Not appended:** the triage advisories (air-cut hotspots, capped and
    deduped per project), and the findings the triage copies from other
    sources (rapid, holder and obstacle collisions; the project
    diagnostics). The list already reads those from their own sources (the
    safety append above it).
  - Cost: one triage build per call.
- `narrate_toolpath` (lead ruling 2026-10-23, done): `mcp_narrate_toolpath`
  appends one sentence, `RoughCoverageReading::narration_sentence`, from
  `ProjectSession::rough_coverage_reading(id, evidence)`. This is the same
  core reading the triage publishes. The sentence is one of three forms:
  - "coverage: no standing island above the stock to leave (…)";
  - "coverage: N island(s) stand more than … mm above the target …; the
    tallest is H mm high and W mm wide at (x, y)";
  - "coverage: not measured (reason)".

  There is no sentence when the op is not a 3D Rough or the stored run is
  not the shown run. Cost: one walk of the op's stock grid. This is more
  than the rest of the narration (which takes about 4 ms).

## Requests (all three done on the lead ruling 2026-10-23)

1. Viz `OWNER_TAB` row: `(i::PROJECT_ROUGH_LEAVES_ISLANDS, L)`. The tab is
   **L (Linking)**, the tab that `link_cuts_stock`,
   `crosses_standing_material`, `entry_load` and `plunge_class_load` use.
   (My first draft said C; the lead ruled to match the neighbours.)
2. `get_toolpath_diagnostics`: see above.
3. `narrate_toolpath`: see above.

## Files changed (mine)

- `crates/rs_cam_core/src/stock/standing_islands.rs` (new)
- `crates/rs_cam_core/src/stock/mod.rs`
- `crates/rs_cam_core/src/stock/sim_triage.rs`
- `crates/rs_cam_core/src/metrology/union_coverage.rs`
- `crates/rs_cam_core/src/session/compute/simulation.rs`
- `crates/rs_cam_core/src/session/compute/diagnostics.rs` (outside the list: wiring)
- `crates/rs_cam_core/src/diagnostics/ids.rs` (outside the list: one id row)
- `crates/rs_cam_core/src/session/compute/export.rs` (ruling 2: the append)
- `crates/rs_cam_viz/src/ui/properties/tab_badges.rs` (ruling 1: one row)
- `crates/rs_cam_viz/src/app/mcp/diagnostics.rs` (ruling 3: one sentence)
- `crates/rs_cam_core/tests/a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs` (new)
- `crates/rs_cam_core/src/stock/CLAUDE.md`, `metrology/CLAUDE.md`, `session/CLAUDE.md`

The other changes in the worktree (`session/compute.rs`, `compute/model_gouge.rs`,
`compute/catalog*`, `adaptive/`, `adaptive3d/`, ...) are editor B's and
editor C's.

## Commands (through `scripts/cargo_lane.sh` in the worktree)

- Sentry: `cargo test -p rs_cam_core -q --test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov`
  - (a) one island: 9 × 21 cells, 7.5 mm, 9 cells wide, class safety.
  - (b) the same block 0.6 mm above the target: no island, no finding, and
    a measured zero published.
  - (c) cell 0.5 > stock to leave 0.25: `NotMeasurable`, no finding.
  - (d) a core where the model has no surface, measured from the
    mesh-bbox floor.
- Unit tests: `cargo test -p rs_cam_core -q --lib standing_islands` and
  `cargo test -p rs_cam_core -q --lib g_roughcov`.
- The union refactor: `cargo test -p rs_cam_core -q --lib audit_separates_standing_clean_and_gouged`
  (metrology is in a test build through the self dev-dependency).
- Unit test of the sentence: `cargo test -p rs_cam_core -q --lib the_narration_sentence_states`.
- Viz: `cargo test -p rs_cam_viz -q --lib _f24` (the badge map), then viz
  `--lib` in full. `controller/tests/rapid_collision_verdict_g_rapidframe.rs`
  calls `diagnose_toolpath_with_evidence`, so it must stay green with the
  append. Then the `rs_cam_mcp` tests and clippy.
  Clippy must also cover the `--no-default-features` path if the lane runs
  it, because `StandingField::walk` has a `parallel` / non-`parallel` pair.

## Falsifiers (live, Phase 0 columns; the lead runs `run_simulation` on a GUI built from this branch)

- **Corne AgentSearch** "3D Rough 1" and "3D Rough 1 (copy)": islands MUST
  be read. Expect hole cores (G-HOLECORE) in about a third of the hex
  holes, each about as tall as the hole depth and a few mm wide. Expect
  the band outside the case wall (G-OUTERBAND) as wide islands. Expect the
  spur at about (104.5, 86.0) with up to 16.5 mm height (G-SPUR), with the
  width limit stated above. The class is safety (taller than 6 / 3 mm per
  pass + one cell).
- **Corne ContourSpiral**: 2 hole cores and the band.
- **Corne ContourParallel**: no hole core and no band. Prediction risk: an
  inside corner of interior angle α in the model, with no fillet of radius
  ≥ R, leaves a residue of area `R²(cot(α/2) − (π − α)/2)` in XY. That is
  1.93 mm² for a 90° corner and 0.48 mm² for a 120° corner with R = 3,
  about 31 or 8 cells at 0.25 mm. This residue reads as an island with a
  width under R. If the zero column reads only such corner islands, that
  is the tool shape and not a coverage defect. The lead decides by look.
  Under this bar they still fail.
- **Abstention check**: a project simulated at a cell above 0.5 mm (Corne
  stock to leave) must read `not_measurable` /
  `cell_coarser_than_stock_to_leave`, not zero.
- **Setup 2** (flipped) ops: `not_measurable` / `setup_not_identity`, by design.

## Risks for the evidence run

1. **Inside-corner residue on Corne ContourParallel.** An inside corner of
   interior angle α in the model, with no fillet of radius ≥ R, leaves a
   residue of area `R²(cot(α/2) − (π − α)/2)` in XY: 1.93 mm² at 90° and
   0.48 mm² at 120° with R = 3. That is about 31 or 8 cells at 0.25 mm,
   about as tall as the corner is deep. Under this bar it reads as an
   island (width under R), so the "zero" column can fail on tool shape and
   not on coverage. Check the island positions against the corners
   before you call it a defect. The lead decides by look.
2. **Tier-map walk on the GUI thread.** A rough whose boundary source is
   `PlannedTierRegions` makes `resolve_generation_inputs` walk the tier map
   (seconds to tens of seconds) for each reading. The triage builds it on
   a GUI cache miss, and `narrate_toolpath` and `get_toolpath_diagnostics`
   build it on every call. All of these run on the GUI thread. The Phase 0
   fixtures use silhouette boundaries, so they do not hit this. If it
   shows, abstain on that source with a stated reason, or memo the reading
   on the run and the result `Arc`.

## Not finished / limits

- No cargo run, so no compile or test result.
- Cost: each triage call resolves the generation inputs of each rough and
  queries `surface_z_at` once per grid column. The GUI caches the triage;
  the MCP calls compute it each time. If it is slow on wanaka200, a memo
  keyed on the run and the result `Arc` is the next step.
- Cost tail: a rough whose boundary source is `PlannedTierRegions` makes
  `resolve_generation_inputs` walk the tier map (seconds to tens of
  seconds) in each triage build. On a cache miss in the GUI, this runs on
  the GUI thread. The Phase 0 fixtures use silhouette boundaries. If the
  lead wants it, abstain on that source with a stated reason.
- Non-identity setups abstain.
- A reading is not a `SimMetric` in `MeasurabilityReport`. Its abstention
  is on `triage.coverage`, not on the measurability strip.
- `get_toolpath_diagnostics` and `narrate_toolpath`: requests 2 and 3.
