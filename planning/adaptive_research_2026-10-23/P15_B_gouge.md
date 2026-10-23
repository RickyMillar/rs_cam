# P1.5 editor B — the gouge instrument for a 3D Rough (G-ROUGHGOUGE)

Worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`,
base a23df1f8. Not built and not run (the verifier runs cargo). Not
committed.

## Deviation from the brief: the slack is the op's chord tolerance

The brief limited the tolerance to two choices: `MODEL_GOUGE_TOL_MM` or
the G-OVERCUTREF simulation cell. I used neither as the Z slack, for this
reason:

- The planner drapes each cut point to the `point_drop_cutter` CL Z of
  the full tool plus the leave (`adaptive3d/path.rs::drape_path_to_leave`,
  spacing at most the tool radius).
- It then simplifies the cut by RDP at `cut_simplify_tolerance` (the
  larger of the op `tolerance`, default 0.1 mm, and the segment merge
  tolerance). Then it blends the corners
  (`adaptive3d/clearing.rs::stamp_emitted_segment`, the emitter mirror).
- So the planner's own promise is: tip >= CL Z + leave − chord tolerance.
  A guard at 0.02 mm (`MODEL_GOUGE_Z_TOL_MM`) would report the
  simplifier as a Blocking SAFETY finding and refuse the export of a
  correct rough.
- The simulation cell is not available: the guard runs at generation,
  before any simulation.

The slack is therefore read from the op config, which the planner also
reads. No new constant is added. The Z comparison stays
`depth > MODEL_GOUGE_Z_TOL_MM`, and the probe radius stays
`R − MODEL_GOUGE_TOL_MM`. To revert to the brief's choice, edit one line:
make `OperationConfig::model_gouge_limit_offset_mm` return
`Some(c.stock_to_leave_axial)`.

## The rule and its derivation

`ModelGougeRule::Allowance { fix }` is a new variant that uses the
existing Surface sampling path (`compute/model_gouge.rs::measure`):

```
limit(x, y) = point_drop_cutter(x, y, reduced tool).z + limit_offset_mm
limit_offset_mm = stock_to_leave_axial − chord_tolerance(tolerance, segment_merge_tolerance)
gouge  <=>  limit − tip_z > MODEL_GOUGE_Z_TOL_MM
```

- **The leave.** `Adaptive3dConfig::stock_to_leave_axial` is the one value
  that `adaptive3d_effective_stock_to_leave` gives the planner. That
  function is `pub(super)` in `compute::execute`, so catalog cannot reach
  it without an edit to `execute.rs`. See the requests below.
- **The chord tolerance.** The new
  `Adaptive3dGeometry::chord_tolerance(tolerance, merge)` holds the rule.
  `cut_simplify_tolerance` now calls it, so the planner and the guard use
  one rule. The session passes the merge tolerance from
  `self.context.dressups.segment_merge`. That is the same expression that
  `execute_generation` gives the planner.
- **No clamp at zero.** At leave 0 the offset is −tolerance, which is the
  planner's promise.
- **The probe.** The probe is unchanged: the session cutter of the tool
  with its diameter less 2 × `MODEL_GOUGE_TOL_MM` (flat, ball, bull nose
  and taper all go through `session_cutter`). The reduced CL Z is at or
  below the full-tool CL Z, so a draped point can never read as a gouge.
  Item 2 of the brief needed no work.
- **The radial leave.** Not judged. The 3D Rough has no radial leave: L2
  deleted the dial, and the planner offsets in Z only. If a radial leave
  comes later, the probe radius must GROW by it to check it. A smaller
  probe would only make the check weaker.
- **The finding.** `ModelGougeFinding` is unchanged (viz tests and h21
  build it as a literal). Under `Allowance`, `surface_z` is the LIMIT Z,
  the same convention as `Bevel`. `deepest_mm` is the depth below the
  limit. The depth into the model is
  `deepest_mm − (leave − chord tolerance)`, where the chord tolerance is
  max(`tolerance`, segment merge tolerance) of THAT toolpath. Segment merge
  is on by default for roughing, so read both from the saved project; the
  value is not always 0.1 mm. `message()` has its own arm,
  which names "the model surface plus the stock to leave (less the chord
  tolerance of the op)".

## The pipeline stage

The guard reads the emitted moves after every stage.
`session/compute.rs::execute_job` calls `phases.guard_model_gouge(&annotated, …)`
directly after `run_stages`. `run_stages` ends with
`apply_dressups` → `clip_and_split_entries`. So the guard sees the toolpath
after the dressups and after the entry split, which is the motion that
export writes.

The finding then flows unchanged:

- `findings.record` → `GenerationFindings::model_gouge` →
  `ToolpathStats::model_gouge`;
- `diagnostics/adapters/from_generation.rs::cuts_into_model` (Blocking,
  `safety.cuts_into_model`);
- `ExportRefusal::CutsIntoModel` through
  `session/compute/export.rs::model_gouge_refusal`.

The guard skips rapids and `ObstacleClearance` moves.

## Cost

Each sample is one `point_drop_cutter` query through the existing
`SpatialIndex`, at a spacing of at most D/4, with both ends of each move.

- A rough of 36 000 moves costs 72 000 queries plus its cut length over
  D/4.
- Example: a 6 mm tool with 50 m of cut costs about 105 000 queries.
  The cost per sample is the same as for a Pocket.
- Not measured. The verifier or the lead reads the "Check against the
  model" phase time on the Corne rough.
- A possible later saving of up to 2×: skip the start sample of a move
  when it equals the previous end. I did not make this change, because it
  changes the `judged` counts of the existing guard.

## Files changed

- `crates/rs_cam_core/src/compute/catalog/capabilities.rs`: the
  `Allowance` variant and `fix_hint`.
- `crates/rs_cam_core/src/compute/model_gouge.rs`: the module doc ("The
  allowance", the cost), `GougeProbe::limit_offset_mm`, the measure arm,
  the message arm, and the `surface_z` doc.
- `crates/rs_cam_core/src/compute/catalog.rs`:
  `OperationConfig::model_gouge_limit_offset_mm` (an exhaustive match).
- `crates/rs_cam_core/src/adaptive3d/mod.rs`:
  `Adaptive3dGeometry::chord_tolerance`; `cut_simplify_tolerance` calls it.
  The behaviour does not change.
- `crates/rs_cam_core/src/session/compute.rs`: `guard_model_gouge` fills
  `limit_offset_mm`.
- `crates/rs_cam_core/src/compute/catalog/registry.rs`: `FIX_ROUGH_3D`,
  and the REG_ADAPTIVE3D row `Exempt` → `Allowance` (edited last).
- `crates/rs_cam_core/tests/a_default_step_pocket_never_cuts_below_a_ring_ux7b.rs`:
  `limit_offset_mm: 0.0` in its two probe literals.
- `crates/rs_cam_core/tests/every_op_states_its_capabilities_w2_4.rs`:
  the frozen `model_gouge` row of Adaptive3d, the `gouge_variant` arm, and
  the module doc.
- `crates/rs_cam_core/tests/a_rough_cuts_no_deeper_than_its_leave_g_roughgouge.rs`:
  new.
- `crates/rs_cam_core/src/compute/CLAUDE.md`: the `model_gouge.rs` entry
  (40 lines).

## The sentry

`a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` has four tests on a
synthetic flat floor at Z 10, a 6 mm flat end mill, and the default 3D
Rough config (leave 0.5 mm, tolerance 0.1 mm, so the offset is 0.4 mm):

1. `a_rough_at_its_leave_reads_clean_g_roughgouge`: a zigzag at the floor
   plus the leave, and at the floor plus the offset, reads clean, with
   `judged > 0` (brief item a).
2. `a_rough_below_its_leave_reads_the_depth_and_the_place_g_roughgouge`
   (brief item b):
   - The zigzag shifted down by leave + 2 × `MODEL_GOUGE_Z_TOL_MM` gouges
     on every sample except the plunge start in the air.
   - The depth is exactly offset + 2 × Z tolerance, and `surface_z` is
     the limit Z.
   - A dip of 1 mm below the floor in one move gives depth 1 + offset, at
     the bottom of the dip, on that move index.
3. `the_offset_moves_only_the_allowance_rule_g_roughgouge`: a tip between
   the floor and the limit gouges under the 3D Rough rule. Under the Pocket
   rule, with the same probe and a non-zero offset, it is clean. Below the
   floor, the Pocket rule reads only the depth below the floor. This shows
   that Surface does not read the offset.
4. `an_allowance_row_has_an_offset_g_roughgouge`: a row states `Allowance`
   exactly when its default config gives an offset. The offset is the
   leave less max(tolerance, merge).

Brief item (c), the 2.5D Surface behaviour on a real wall, stays with the
existing
`a_default_step_pocket_never_cuts_below_a_ring_ux7b::a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b`
(it needs the `step` feature).

Commands:

```
cargo test -p rs_cam_core -q --test a_rough_cuts_no_deeper_than_its_leave_g_roughgouge
cargo test -p rs_cam_core -q --test every_op_states_its_capabilities_w2_4
cargo test -p rs_cam_core -q --features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b
```

I did not inject a defect to see the sentry go red. To check it, set
`limit_offset_mm` to `0.0` in the session or in the measure arm. Claim 3
must then fail.

## Falsifiers (the lead's evidence run)

- **Corne ContourParallel "3D Rough 1": expect 0 findings.** One
  mechanism can still give a finding, and it is not RDP sag. Near a
  vertical wall, the chord between a floor-level drape point and a point
  that the drape lifted can let the tool edge overlap the wall by up to
  one sample step for a short distance. When `min_cutting_radius` is set
  (default 0.0, a no-op), the corner blend (`blend_corners_3d`) also moves
  points in XY after the drape. The guard
  reports such a radial overlap as a Z depth of about the full wall
  height. If it fires, read `at` against the nearest wall in XY. A large
  `deepest_mm` with `at` within R of a wall is a true radial micro-gouge,
  not an instrument fault.
- **Corne AgentSearch "3D Rough 1 (copy)", move 1401
  (`crosses_standing_material` peak 15 mm): expect NO model finding
  there.** `crosses_standing_material` is a stock reading: a link through
  material that stands above the model. The emitter drapes links
  (`Adaptive3dSegment::Link` → `drape_path_to_leave`) to CL + leave, so a
  link through stock stays above the model. A finding at 1401 would mean
  that a link chord dips between drape points, or that a dressup after
  the planner moved it. Both would be real.
- **G-WALLDIP (Corne CP copy, the dips to Z −7.5 along the outer wall):**
  this is the best place to look for a real positive. If the dips go below
  the case wall's CL + 0.4, the guard reports them at the dip bottoms.

## Blast radius (for the verifier)

The 3D Rough now records a Blocking finding when it gouges. Any test that
generates a 3D Rough on a mesh and then exports, or that asserts zero
Blocking diagnostics, turns red if the rough really dips below CL + leave −
tol. My search of `tests/` found no test that both names Adaptive3d and
exports or asserts Blocking. These fixtures hold a 3D Rough and are the
ones to watch:

- `tests/fixtures/wanaka_*.toml`
- `testpiece_mini_2026-10-20_c3808eb0`
- `op_golden_g_opgolden.toml` (the golden does not hold `model_gouge`)
- `volumetric_produced_w6_5.toml`
- `test_job.toml` (CLI)
- the CLI smoke and integration exports

Run the viz `--lib` and the `rs_cam_mcp` tests (memory: agent gates
include viz `--lib`). No viz source matches on `ModelGougeRule`.

## Other 3D rows that could take the rule later (not changed)

- With a `stock_to_leave` dial (`OperationConfig::stock_to_leave_dial`):
  Pencil, Scallop, UnifiedFinish, SteepShallow, RampFinish, SpiralFinish,
  RadialFinish, HorizontalFinish. Each needs its own chord tolerance term.
  Scallop and UnifiedFinish state the leave normal to the surface
  (`stock_to_leave · cos θ` convention, `operation_configs.rs` ~2102).
  For those the vertical limit is not CL + leave on a slope; derive it
  before you move the row.
- DropCutter and Waterline (no dial): `Allowance` with the offset −
  (their chord tolerance) is a candidate, or plain `Surface` if their
  emitters hold CL exactly.
- Keep these `Exempt`: ProjectCurve (it cuts below the surface by design;
  it is closer to `Bevel`, with its commanded depth), VCarve, Inlay,
  InlayPlug, Trace, AlignmentPinDrill.

## Requests and things not finished

- **Request (owner of `compute/execute.rs`):** make
  `rough_3d::adaptive3d_effective_stock_to_leave` reachable from
  `compute::catalog` (for example `pub(crate) use`). Then
  `model_gouge_limit_offset_mm` can call it instead of reading
  `stock_to_leave_axial` directly. Today the two read the same field.
- The session fill of `limit_offset_mm` (from
  `self.context.dressups.segment_merge`) has no test. The sentry calls
  `measure` directly. Only the Phase 0 evidence run covers the session
  door.
- The drape cutter is the real tool (`ctx.tool_def` →
  `adaptive_3d_toolpath_output_with_cancel`), and the probe is the session
  cutter of the same tool, made smaller. Checked by reading; no taper case
  is in the sentry.
- Peer editors work in the same worktree. At the time of this report,
  `git status` also shows their files (`adaptive/`, `stock/`, `metrology/`,
  `diagnostics/ids.rs`, `session/compute/{diagnostics,simulation}.rs`,
  `finish/scallop_isofield.rs`, and two new tests). Those are not mine.
  My files are listed under "Files changed", plus `adaptive3d/CLAUDE.md`
  (one line: `chord_tolerance` is the one RDP rule).
- The guard does not add an arc-fitting tolerance. If arc fitting runs on
  a rough after the planner, its deviation is not in the slack.
- Not measured: the guard time on a real rough, and whether the sentry
  goes red on an injected defect (the verifier).
- `ModelGougeFinding` does not carry the offset, so the message gives the
  limit Z, not the depth into the model. The report formula above gives
  that depth.
