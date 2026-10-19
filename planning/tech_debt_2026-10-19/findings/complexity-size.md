# Findings: complexity-size (size and complexity)

Evidence baseline 99b6fb41, branch geo-select, read on 2026-10-19.
Method: `rg` and file reads only. Function lengths come from a brace-matched
scan of every production `.rs` file under `crates/*/src`. The scan skips
`tests.rs`, `*_tests.rs`, `tests/` folders and the inline `#[cfg(test)] mod`
tail. Caller counts come from `rg -w`.

Census (brace-matched, production only):

- 564 production files. 42 files have more than 1500 production lines, and
  18 files have more than 2000.
- 180 functions have 150 lines or more, 67 have 250 lines or more, and 14
  have 500 lines or more (core 36, viz 28, cli 3 at 250 or more).
- 136 `allow(clippy::too_many_arguments)`, 8 `type_complexity` (2 in
  production), 3 `too_many_lines` (all in tests). No `clippy.toml` exists.

Known debt: T-5 (`feeds::calculate`) and Q10 / Q11 of the 2026-09-16 plan.
That plan ruled "long functions are not split mechanically". The fixes below
are structural (a type or a table), not mechanical splits.

---

## CS-1 (B): the dressup order lives in the statement order of a 470-line function, and `DRESSUP_PIPELINE` is read only by a test

**Sites**

- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:253-264` (`DRESSUP_PIPELINE`)
- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:471-940` (`apply_dressups`, 470 lines; stage calls at :517, :612, :648, :676, :702, :720, :761, :788, :821, :849, :873, :917)
- `crates/rs_cam_core/tests/dressup_span_invariants.rs:438-536` (the only reader of the list)
- `crates/rs_cam_core/src/compute/execute.rs:557` (re-export)

**Claim.** The doc at `dressup_apply.rs:240-252` says "ORDER IS LOAD-BEARING
and this list states it". Production never reads the list. The real order is
the sequence of `if` blocks inside `apply_dressups`. The two copies must agree
by hand. The sentry checks a subsequence only for the configurations that it
builds.

**Evidence.** `rg DRESSUP_PIPELINE` gives the definition, the re-export and
the test only. Cold-start task 3 recorded this as a silent miss: "two separate
lists of the same order ... a stage placed in the wrong spot in the body or
missing from the list is caught only if the test config enables it". The
function also holds about 35 lines of campaign history comment
(`:538-573`), so the stage order is hard to read in the body.

**Fix.** Make the table drive the body. Turn `DressupStage` into an enum (or
give each const a `run` fn pointer) and write
`for stage in DRESSUP_PIPELINE { match stage.kind { ... } }`. An exhaustive
`match` forces a new stage to state its arm, and the list becomes the one
order. Move each arm body into its own `fn` that takes `&DressupContext`.

**Enforcement.** Now SENTRY (partial). After the fix TYPES. Cost M. In flux.

---

## CS-2 (D): 27 signatures pass a radial LUT and a separate radius, and one kernel reads both

**Sites**

- `crates/rs_cam_core/src/dexel_stock/simulation.rs:66-70, 85-89, 126-131, 361-365, 414-418, 907` (9 signatures in the file)
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:783` (scan box from `radius`) and `:809` (coverage from `lut.radius_sq()`), in one function
- `crates/rs_cam_core/src/dexel_stock/swept.rs` (4 signatures; `:332`, `:493` read `lut.radius_sq()`)
- `crates/rs_cam_core/src/dexel_stock/stamping.rs` (4), `dexel_stock/mod.rs` (3), `dexel_stock/playback.rs` (1, `:280` reads `lut.radius_sq()`)
- `crates/rs_cam_core/src/adaptive3d/clearing.rs` (5; `:641`, `:722`), `crates/rs_cam_core/src/compute/simulate.rs:1583`
- `crates/rs_cam_core/src/stock/radial_profile.rs:23-37` (`RadialProfileLUT` stores `radius_sq`)

**Claim.** `RadialProfileLUT::from_cutter` already stores the cutter radius
(`radius_sq`). The stamping API still takes `lut` and `radius` as two
positional arguments. All callers today pass `cutter.radius()` from the same
cutter that built the LUT. A caller that passes a different radius compiles.
The kernel at `stamping.rs:783-809` then sizes the cell scan with one radius
and tests coverage with the other, and the stamp is clipped with no error.

**Evidence.** `rg -A1 "lut: &RadialProfileLUT,$"` followed by `radius` gives
27 signatures in 7 files. `rg "lut.radius_sq()"` gives 9 production reads
inside the same call tree.

**Fix.** Add `RadialProfileLUT::radius()` and delete the `radius` parameter
from every stamping and simulation signature. If a caller must hold a
different reach, give that caller a named type (for example `ProbeRadius`)
and do not reuse `f64`.

**Enforcement.** Now NONE (convention: the caller derives both from one
cutter). After the fix TYPES. Cost M.

---

## CS-3 (E): entry-point ladders: each new knob adds a sibling function with one more positional argument

**Sites**

- `crates/rs_cam_core/src/dexel_stock/simulation.rs:39-907` (9 `simulate_*` variants; 13 `too_many_arguments` allows; the metric chain has 15 and 16 positional arguments at `:323`, `:361`, `:414`)
- `crates/rs_cam_core/src/compute/simulate.rs:1759-1776` (production call: `entry_rpm, entry.flute_count` are two adjacent `u32`, `rapid_feed_mm_min, sample_step_mm` two adjacent `f64`, and a bare `true`)
- `crates/rs_cam_core/src/finish/scallop.rs:1060, 1099, 1138` and `crates/rs_cam_core/src/finish/scallop/research.rs:127, 215` (5 rungs; the production body is the 665-line `scallop_toolpath_research_with_link` in a file named `research.rs`)
- `crates/rs_cam_core/src/adaptive3d/mod.rs:499, 515, 547, 571, 618, 634` (6 rungs; `:515` has no production caller)
- `crates/rs_cam_core/src/finish/finish_setup.rs:380, 422, 448, 521, 545, 571` (6 rungs; `:422`, `:448`, `:521` have no production caller)
- `crates/rs_cam_core/src/finish/ramp_finish.rs:501, 531`

**Claim.** The code states the mechanism itself.
`simulation.rs:82-84`: "The sibling exists rather than a widened signature so
every other caller ... keeps its call site". `scallop.rs:1092-1097`: the link
stage "is NOT a `ScallopParams` field" because the struct "is built by
literal" in a dozen places. Each rung adds one `Option` or policy argument.
The chains carry same-type adjacent scalars and bare `bool` literals, so a
swap compiles. Test-only rungs collect at the low end of each ladder. The
production scallop body sits in `scallop/research.rs`, but
`finish/CLAUDE.md` says that "Research" means `--features research`.

**Evidence.** Caller counts by `rg -w` (prod / all):
`build_finish_surface_with_cancel` 0/10,
`build_classification_surface_with_cancel` 0/26,
`build_finish_surface_with_cell_size_and_cancel` 0/13,
`adaptive_3d_toolpath_traced_with_cancel` 0/1,
`simulate_toolpath_with_cancel` 1/1. A past break of the same shape is
recorded at `simulation.rs:107-111`: two copies of the playback loop drifted,
and "the range one was already missing the per-arc-window structure". The
repo already has the fix pattern: `DressupContext`
(`dressup_apply.rs:421-430`, "took 15 positional arguments") and
`EntrySafety`.

**Fix.** Give each engine one request type with defaulted options, for
example `SimRequest { toolpath, profile, direction, cancel, metrics:
Option<MetricInputs>, rapid_check: Option<..> }` and a `ScallopRun` with
`..ScallopRun::new(mesh, index, cutter, params)`. Put `spindle_rpm`,
`flute_count` and `rapid_feed_mm_min` in a named struct. Replace the bare
`bool` with an enum. Delete the test-only rungs. Rename `scallop/research.rs`
to say what it holds (for example `scallop/pipeline.rs`). This is a type
change, not a mechanical split.

**Enforcement.** Now CONVENTION (the clippy lint is allowed at each site).
After the fix TYPES. Cost L. Known: Q10 (2026-09-16) graded it F "style". The
count is now 136 (Q10 said 110).

---

## CS-4 (E): `feeds::calculate` is 1321 lines, with 23 mutable locals across 19 step banners (T-5)

**Sites**

- `crates/rs_cam_core/src/feeds/mod.rs:1494-2814`
- `crates/rs_cam_core/src/feeds/mod.rs:1530` (comment: Step 5 rebinds `effective_d`)

**Claim.** One function holds RPM, chip load, the speed-up, the tier
ceiling, the drill clamp, DOC/WOC, the flute and shank guards, the feed, the
setup derates, the power ladder (five rungs), the machine ceiling, the plunge
and the ramp. 627 of its 1321 lines are comments. Steps refer forward and
backward ("Step-9b rubbing floor below catches the fall", "Step 7 used to
ship"). A reader cannot see which locals a step reads.

**Evidence.** Brace-matched length 1321 (T-5 said 1302). `let mut` count 23
(T-5 said 16). The banners run `Step 1` to `Step 8b` with lettered sub-steps.
What changed since T-5: the function grew by 19 lines and 7 mutable locals.
`suggest.rs` fell from 5667 to 1067 lines because it split into `suggest/`.
`feeds/mod.rs` is 3092 lines (T-5 said 4144).

**Fix.** Give each step an input type and an output type, for example
`RpmStage -> ChiploadStage -> EngagementStage -> FeedStage -> PowerStage`.
Each stage is a `fn` that takes the previous output by value. A later step
then cannot read a value that an earlier step has not yet produced, and the
`effective_d` rebind becomes two named fields.

**Enforcement.** Now NONE. After the fix TYPES. Cost L. Owner: power-calcs
(T-5). Not in flux.

---

## CS-5 (E): `upload_gpu_data` is one 1327-line function with about twelve keyed upload slots inline

**Sites**

- `crates/rs_cam_viz/src/app/gpu_upload.rs:155-1481` (keys built at `:218`, `:263`, `:269`, `:662-692`; collision slot `:821-834` keeps its own compare; `type_complexity` allow at `:986`)
- `crates/rs_cam_viz/src/render/upload_cache.rs` (the key types)

**Claim.** Each slot builds a content key from some inputs and then rebuilds
from inputs that it reads again, further down the same function. Nothing ties
the key to the inputs of the rebuild. An input that the rebuild reads and the
key omits gives a stale buffer with no error. The function is too long for a
reader to check this pairing.

**Evidence.** Brace-matched length 1327. The key-versus-input class has broken
before: `4e8e4db4` (weak-pin the V8 upload keys), `63b8ed1d` (SHL-05, one
keyed compare helper), and `c84ef3be` ("caches key on the inputs they read,
not edit_counter").

**Fix.** Give each slot a type with `fn inputs(&state) -> Inputs`,
`fn key(&Inputs) -> Key` and `fn build(&Inputs) -> Buffer`. The rebuild then
can read only what the key saw. `upload_gpu_data` becomes a loop over slots.

**Enforcement.** Now CONVENTION. After the fix TYPES. Cost M. In flux.

---

## CS-6 (E): grids and bounding boxes travel as anonymous tuples

**Sites**

- `crates/rs_cam_core/src/adaptive3d/clearing.rs:419-425` (`build_material_bool_grid` returns `(Vec<bool>, usize, usize, f64, f64, f64)`; destructured at `:1060`, `:1237` as `(cleanup_grid, cr, cc, co_x, co_y, c_cs)`, `:1432`, `:1880`)
- `crates/rs_cam_core/src/adaptive3d/path.rs:572-579` (an 8-tuple `Result<(Vec<f32>, usize, usize, f64, f64, f64, f64, f64), _>` behind a `type_complexity` allow)
- `crates/rs_cam_core/src/polygon.rs:1750` and `crates/rs_cam_core/src/adaptive/material_grid.rs:655` (two identical `polygon_bbox -> (f64, f64, f64, f64)`)
- `crates/rs_cam_core/src/finish/finish_planner.rs:1354`, `crates/rs_cam_core/src/geometry/region_mask.rs:344` (two more tuple bboxes, in tests)
- `crates/rs_cam_core/src/polygon.rs:186` (`Polygon2::bbox -> [f64; 4]`, a third shape)
- `crates/rs_cam_core/src/geometry/grid2.rs:75` (`Grid2` names its sides `nx`, `ny`; the tuple above says rows, cols)

**Claim.** A grid is `(cells, rows, cols, origin_x, origin_y, cell_size)` by
position. Rows and cols are both `usize`, and the three trailing values are
all `f64`, so a swap at any of the four call sites compiles. The bbox has
three shapes (two tuple functions with the same body and one array).

**Evidence.** `rg "\) -> (Result<)?\(([^()]*,){3,}"` gives 15 production
returns of four or more tuple fields. The two `polygon_bbox` bodies are the
same text.

**Fix.** Return a named struct (`MaterialMask { cells: Grid2<bool>, origin:
P2, cell_size: f64 }`). Add one `Bbox2 { min: P2, max: P2 }` and make
`Polygon2::bbox` return it. Delete both `polygon_bbox` copies.

**Enforcement.** Now NONE. After the fix TYPES. Cost S to M. `polygon.rs` is
in flux.

---

## CS-7 (E): the debt instrument measures function and file size wrongly, and the 09-16 plan inherited the errors

**Sites**

- `planning/tech_debt_2026-10-19/instruments/inventory.md` ("Largest production files", "Functions >= 250 lines (crude: to the next `fn`)")
- `scripts/debt_scan.py` (the producer; in the git status as modified)
- `planning/tech_debt_2026-09-16/findings/Q_allows_todos_size.md:178-190` (Q11 top-10 list)

**Claim.** The function scan measures "to the next `fn`". It reports data
tables and gaps as functions:

- `in_simulation` (`overlays/registry.rs:490`) is a 3-line fn followed by the `ROWS` table; the scan says 910.
- `from_stock_bbox` (`feeds/suggest.rs:49`) is 10 lines; the scan says 705.
- `fmt` (`session/command.rs:986`) and `fmt` / `from` (`rs_cam_mcp/src/server.rs:78`, `:726`, `:1194`) say 1086, 648, 468 and 465.

The "production files" table includes test files and inline test tails:

- `compute/worker/tests.rs` is the first row.
- `gcode/mod.rs` shows 3231 lines, but only 1620 are production.
- `tool_load/verdict.rs` shows 3344 lines, but only 2236 are production.

Q11 (2026-09-16) ranked `strip_all` (1015) and `fmt` (1048). `strip_all` is a
`const fn` at `compute/catalog/schema.rs:569`.

**Evidence.** The brace-matched rescan in this review: 67 functions of 250
lines or more (the instrument says 91), and 42 production files over 1500
lines.

**Fix.** Make `debt_scan.py` brace-match each body and cut each file at its
`#[cfg(test)]` module. Then store the census in a checked-in budget file (see
CS-8).

**Enforcement.** Now NONE. After the fix SENTRY. Cost S. Known: Q11 (the list
it printed). `scripts/debt_scan.py` is modified in the working tree.

---

## CS-8 (F): no size budget exists, and 67 production functions are 250 lines or more

**Sites** (not covered by CS-1 to CS-5; brace-matched lengths)

- `crates/rs_cam_core/src/adaptive/path.rs:671` `adaptive_segments_with_debug` 827 (25 `let mut`, 10 indent levels)
- `crates/rs_cam_core/src/tool_load/chipload.rs:372` `evaluate_inner` 640
- `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:471` `draw_geometry_tab` 590 (in flux)
- `crates/rs_cam_core/src/session/compute/diagnostics.rs:922` `diagnostics_with_evidence` 589 (in flux)
- `crates/rs_cam_core/src/export/viz.rs:580` `stacked_simulation_3d_html` 570 (one caller, `rs_cam_cli/src/job.rs:588`)
- `crates/rs_cam_core/src/adaptive3d/path.rs:606` `adaptive_3d_segments` 518
- `crates/rs_cam_viz/src/render/mod.rs:263` `RenderResources::new` 512 and `:929` `prepare` 455 (`render/CLAUDE.md` calls `mod.rs` "the pipeline facade")
- `crates/rs_cam_core/src/feeds/rationale.rs:375` `entry_for_warning` 511
- `crates/rs_cam_core/src/finish/surface_link.rs:865` `relink_fragments_with_kinds` 508
- `crates/rs_cam_cli/src/project.rs:635` `run_project_command` 498
- `crates/rs_cam_viz/src/app.rs:860` `draw_frame` 482 (in flux)
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2377` `clear_one_region` 466
- `crates/rs_cam_core/src/compute/simulate.rs:1912` `run_simulation_memoized` 445 (in flux)
- `crates/rs_cam_viz/src/ui/properties/mod.rs:170` (a stray `#[allow(clippy::too_many_arguments)]` above the `ToolpathTab` enum; it does nothing; in flux)

**Claim.** Nothing stops a function or a file from growing. The workspace
lint table (`Cargo.toml:35-62`) has no `too_many_lines`, and no `clippy.toml`
sets a threshold. The three `too_many_lines` allows are in tests
(`spiral_finish_compact_c1.rs:729`, `strategy_comparison_h4.rs:1176`,
`rapid_replay_shipped_gcode_s1.rs:998`), where the lint is off, so they do
nothing.

**Evidence.** The census above. Functions of 150 lines or more: 180. Most of
the top functions mix a planner loop with diagnostics and trace capture.

**Fix.** Add a ratchet, not a blanket lint. Store a budget file with the
current size of each function of 250 lines or more and of each file over
1500 production lines. A sentry fails when a new entry appears or an entry
grows. Delete the no-op allows. Split only at a stage boundary, with a type
for each stage (see CS-4), when the owner next changes the function.

**Enforcement.** Now NONE. After the fix SENTRY (LINT is possible with
`too_many_lines` and a threshold, but 180 sites would need allows). Cost S for
the ratchet. Known: Q11 (2026-09-16), "no mechanical split proposed".

---

## CS-9 (F): `app/mcp/commands.rs` is 3197 lines, with `describe_core` at 899 and `core_command_for` at 505

**Sites**

- `crates/rs_cam_viz/src/app/mcp/commands.rs:2031-2929` (`describe_core`)
- `crates/rs_cam_viz/src/app/mcp/commands.rs:335-839` (`core_command_for`)
- `crates/rs_cam_viz/src/app/mcp/commands/selection.rs` (new, untracked: the P2.7 split has started)
- `crates/rs_cam_viz/src/app/mcp/commands/obstacles.rs`, `commands/stock_changes.rs` (earlier splits)

**Claim.** Both functions are exhaustive matches over `CommandId`
(`commands.rs:2026-2029`: "a new row still has to state what it does here
before it compiles"), so coverage is TYPES-enforced. The defect is size only:
most arm bodies are inline. The file mixes conversion, toast text, reply shape
and refusal shape.

**Evidence.** Brace-matched lengths. The `describe_*` helpers that already
exist (`describe_stock_change`, `describe_obstacle`,
`describe_edit_selection`) show the target shape.

**Fix.** Keep the two matches. Make every arm one call into a per-family file
under `commands/`, as `obstacles.rs`, `stock_changes.rs` and `selection.rs` do.

**Enforcement.** Now TYPES (coverage). After the fix TYPES. Cost M. In flux.

---

## CS-10 (F): `controller/events/compute.rs` (2662 lines) holds the generate-all plan driver and a JSON diagnostics builder beside the result adopters

**Sites**

- `crates/rs_cam_viz/src/controller/events/compute.rs:1547-2173` (plan driver: `plan_steps`, `mcp_start_generate_all`, `start_plan`, `pump_plan`, `submit_plan_step`, `finish_plan`, `stop_plan`, `cancel_generation_plan`, and more)
- `crates/rs_cam_viz/src/controller/events/compute.rs:2345-2580` (`build_mcp_diagnostics`, 236 lines of `serde_json::Value`)
- `crates/rs_cam_viz/src/controller/events/compute.rs:675` (`adopt_simulation_result`, 382 lines)
- `crates/rs_cam_viz/src/controller/generate_all.rs` (the plan types already live here)

**Claim.** The file has three jobs. The plan types live in
`controller/generate_all.rs`, but their driver is about 620 lines in
`events/compute.rs`. The MCP JSON builder is a wire concern inside the
controller.

**Evidence.** The function list (`rg -n "fn \w+"` on the file). This finding
matches the viz-controller-app map, section 8 ("mixes submit, four result
adopters, the plan driver, MCP waiter notification and a JSON diagnostics
builder").

**Fix.** Move the plan driver into an `impl` block in
`controller/generate_all.rs` (or `events/plan.rs`). Move
`build_mcp_diagnostics` to `app/mcp/`. This is a move with no change in
behaviour.

**Enforcement.** Now CONVENTION. After the fix CONVENTION. Cost S. Not in flux.

---

## Coverage

Swept: every production `.rs` file under `crates/*/src` (all six crates) with a
brace-matched function-length and file-length scan. Also swept:

- the `allow(clippy::too_many_arguments | type_complexity | too_many_lines)` census
- adjacent same-type parameter runs, long tuple returns, deep nesting, and `impl` spread
- the 10 maps, `CHECKLIST.md` section 10, the instruments, the cold-start tasks, the register and the 09-16 plan

Read in full: `dexel_stock/simulation.rs` signatures, `finish/scallop.rs`
entry rungs, `scallop/research.rs` header, `dressup_apply.rs` pipeline,
`feeds::calculate` banners, and the `upload_gpu_data` slot structure.

Checked and not filed:

- `ProjectSession` has `impl` blocks in 24 files, but it has no public `&mut self` method outside `apply` and the builder, so the mutation contract holds.
- Deep nesting is local (at most 14 matches in a file at 24 or more spaces).
- `session/command.rs` (2840 lines) is a declarative row table.
- The research arms (`conformal_spiral.rs`, `direction_field.rs`, `spiral_finish_compact.rs`, 5990 lines) are behind `--features research`.

Not done:

- No per-arm read of `describe_core` or `core_command_for`.
- No check of the 67 long functions for defects inside them.
- The test files (for example `compute/worker/tests.rs`, 3476 lines) are out of scope.
- No cargo run, so no clippy pedantic counts.
