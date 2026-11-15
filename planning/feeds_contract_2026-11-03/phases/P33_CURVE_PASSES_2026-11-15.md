# P33: a depth per pass inside Project Curve (2026-11-15)

Operator ruling 2026-11-15 (`P31_OPERATOR_RULINGS_2026-11-15.md` section E, ruling 3): "For
multiple passes that should be built into the toolpath, not multiple tool paths!". The Project Curve
op gets a depth per pass.

This file holds the rules and the predictions first (sections 1 to 4, written before the code),
then the results (section 5 on).

## 1. The pass convention, from history

- `git log -p -S'pass 1 of 7' -- planning/rivmap_block_job/` gives no commit, on every ref. No
  stored file holds a pass 1 to 6. The operator file holds only "River channel 6 mm, pass 7 of 7"
  (`depth = -4.0`, from above, cutter projection, `top_z` manual 16.5 = the cavity floor).
- The convention is the loop of `planning/rivmap_block_job/derive_wanaka21.py` lines 646 to 664
  (commit `a73803f70`): `for k in range(n_ch - 1, -1, -1)`, pass `n_ch - k` of `n_ch` has
  `depth = -(web + k * ch_step)`.
  - Each pass is the SAME projected curve, `k` steps away from the full depth. It is not a flat level.
  - A pass covers every curve of the layer. The deepest pass (`k = 0`) is the last pass.
  - The count comes from the deepest channel below the op top Z
    (`channel_depth = cav_floor - (lowest sheet Z + web)`), and `heights.top_z = cav_floor`.
  - The script comment: "where a level is above the cavity floor it cuts air".

## 2. The rules

1. New key `depth_per_pass` on Project Curve, optional. No key: one pass, the path of today,
   byte for byte. No file gains the key on a save.
2. The ladder is the crate's one ladder, `ops::depth::z_ladder` (`LadderMode::EvenRedistributed`),
   from the op top Z (`heights.top_z`) down to the lowest tip Z of the full-depth path of every
   curve of the op: `n = ceil(total / depth_per_pass)` equal steps. No second ladder.
3. Pass `i` is the full-depth path moved up by `level_i - lowest tip Z`. The last pass is the
   full-depth path itself.
4. A sample of an earlier pass at or above the op top Z is left out, and the chain splits there (as
   it does at a gap). So an earlier pass does not feed above the op top, and each pass cuts at most
   one step below the op top or below the pass before it.
5. Order: pass-major, as the history loop. Pass 1 covers every curve, then pass 2.
6. Entry of each later pass: the entry of the first pass. A rapid at the retract plane to the chain
   start, a straight plunge at the plunge rate (`EntryPlunge`), the cut, a rapid retract. The plunge
   goes through the slot of the pass before and then at most one step into material. The dressup
   stage sees each chain as it sees a chain of today.
7. The chain link stage (`chain_distance_mm`) runs on each pass alone. No link joins two passes.
   The rapid-order dressup gets a barrier before each later pass.
8. The stated cut: with the key, `Axial::PerPass(depth_per_pass)`, so the declared body depth is
   `Stated(depth_per_pass)`. With no key nothing moves.
9. No feeds door starts a ladder. The feeds setter of the op writes the key only when the op
   already states it; the operator states it through the parameter door or the project file.

Departures from the history convention:

- History used a constant step from the full depth up (`k * ch_step`). The one ladder gives equal
  steps `total / n`, each at or below the stated depth per pass.
- History fed the passes in air above the cavity floor. Rule 4 leaves those samples out.

## 3. What the cut object states today (found in the code, before the change)

- `compute/catalog.rs` (the `ProjectCurve` arm of `cut_spec`): `axial: Axial::SurfaceOffset(depth)`,
  `total: None(DEPTH_AFTER_SIMULATION)`.
- `compute/catalog/cuts.rs:1206-1214` (`declared_depth`): `DeclaredDepth::AtLeast(TipDepth)`.
- `compute/catalog/cuts.rs:156-168` (`TipDepth::lower_bound_mm`): a depth at or below 0 from above
  gives `None`.
- `feeds/planned.rs:315-317`: then `BodyDepth::None(DepthAfterSimulation)`. No row before the
  simulation holds a depth.
- The river pass has `depth = -4.0` from above. So `ProjectSession::stated_cut_over_a_limit`
  (`session/limit_rows.rs`) finds no stated row with a depth, and `export.stated_cut_over_limit`
  does not come for the 14.3 mm pass. The op states NO axial depth for a tip at or above the model
  surface, and a lower bound for a tip below it.

## 4. Predictions (before the code)

| # | Prediction |
|---|---|
| P-A1 | No key: the op golden does not move (the known 46 red rows, the same rows). |
| P-A2 | Flat plate at Z 0 = the op top, depth 3.0, depth per pass 1.0, 8 chains: 3 passes, 24 chains in the order 8 at Z -1, 8 at Z -2, 8 at Z -3; 24 plunges; every rapid at the retract plane; the last pass equals the no-key path. |
| P-A3 | Surface at Z 0, depth -4.0 (tip at Z 4), op top Z 10, depth per pass 2.5: `ceil(6 / 2.5) = 3` steps of 2.0, passes at Z 8, 6, 4. |
| P-A4 | A V valley: each earlier pass has fewer fed points than the last pass, no fed point of an earlier pass is at or above the op top, and each fed point is within one step of the op top or of the pass before it. |
| P-A5 | The stated cut: `Axial::PerPass` with the key, `Axial::SurfaceOffset` with no key. |
| P-A6 | The MCP wire snapshot moves by the one parameter `depth_per_pass` of `project_curve`. |
| P-A7 | FM1 does not move (no FM1 cell holds the key). |
| P-A8 | The red test: before the code a config with the key cuts 8 chains at Z -3 (one pass). |

## 5. What is built (part A)

- `ProjectCurveConfig::depth_per_pass: Option<f64>` (range 0.1 to 20.0 mm, the range of the Trace
  depth per pass; not written when empty). The GUI row "Depth per pass" is under "Depth"; empty
  shows "one pass".
- `ops/project_curve.rs`: `PassLadder`, `ladder_passes` (rules 2 to 4 and 6). It reads the fed
  chains of the full-depth path and calls `z_ladder`.
- `compute/execute/curve.rs`: `CurveOp::passes` (default: one pass). `run_curve` links each pass
  alone, adds the link totals (`RelinkTotals::absorb`) and puts a `RapidOrderBarrier` span before
  each later pass. With one pass the runner makes the one `link` call and the spans of before.
- `compute/catalog.rs`: the cut spec of a Project Curve with the key is `Axial::PerPass`.
- The feeds role (`impl_operation_params!`, an `extra` block): the getter gives the key; the feeds
  setter writes only a stated key; `has_depth_per_pass` stays `false`.
- `session/compute/params.rs`: the named `depth_per_pass` arm and its `null` arm take only an op
  with the feeds role. A Project Curve takes the serde arm, which writes the key, checks the range
  and clears it with `null`.

Which entry each later pass uses: the same as the first pass (rule 6). `emit_path_segment_with_intent`
gives each chain of each pass a rapid at the retract plane, one straight plunge at the op plunge
rate and a rapid retract. After the registry dressups on solid stock no rapid ends below the stock
top, and no chain leaves its pass.

Through the session door (`a_project_curve_ladder_holds_through_the_session_p33`): the key set by
`SetToolpathParam`, the registry dressups, a boundary that cuts the curve at x = +/- 5 mm and the
entry step. At each XY of the stored result no deeper cut comes before a shallower cut, the last cut
is the cut of the op with no key, and no rapid ends below the stock top. The session clips after the
dressups (`session/compute.rs:758-759`), so the rapid-order dressup reads the barriers of the runner.

The stated rows of a keyed op (the same file, claim 2): a tip 1 mm above the model surface with no
key gives no force budget number of the body cut and no flute guard number ("depth not modelled");
with `depth_per_pass = 2.0` the rows read "body · depth 11 % of force cap" and "stated point · depth
10.0 % of flute guard cap" on the fixture.

The serde arm of the setter stamps no manual provenance for the key (the named arm stamps
`FeedsField::DepthPerPass`). No Suggest door reads a stamp for a key with no feeds role.

Not changed: `commanded_depth_mm` and the engagement guard (`BuriedCurveFinding`). The guard reads
the full-depth path against the stock before the op, as before.

A limit of rule 4: where a chain of an earlier pass starts or stops, the cut is up to one step plus
the Z change of the surface over one sample step.

## 6. Results against the predictions (part A)

| # | Result |
|---|---|
| P-A1 | Held. `every_op_keeps_its_toolpath_g_opgolden`: 46 changes, the 46 rows of `opgolden_known_46_rows.txt` (sorted diff empty); the seven `project_curve/*` arms read "ok". |
| P-A2 | Held. `a_depth_per_pass_cuts_the_curve_in_passes_p33`. |
| P-A3 | Held. `the_ladder_steps_from_the_op_top_to_the_full_depth_p33`: passes at Z 8, 6, 4. |
| P-A4 | Held. `no_pass_cuts_more_than_one_step_p33`: 4 passes on the valley. |
| P-A5 | Held. `the_stated_cut_is_one_pass_and_no_feeds_door_starts_a_ladder_p33`. |
| P-A6 | WRONG. The MCP wire snapshot (`mcp_wire_surface_pin`, 17 tests) did not move. It holds the tools and their arguments, and no operation parameter. `get_operation_schema` serves the new parameter from the registry. |
| P-A7 | Held. FM1 2 passed, no CSV moved. |
| P-A8 | Held. Before the code: `left: 8, right: 24`, cut Z `[-3.0; 8]`. |

Not predicted, found by the sentries:

- `set_param_refuses_absent_field_n5`: the population that refuses `depth_per_pass` goes from 17 to
  16 ops (Project Curve publishes the name). The pin and the arm marker of the source scan moved.
- `every_op_conforms_w4` check 6b: `depth_per_pass = null` was refused by the tool-derived `null`
  arm; that arm now takes only an op with the feeds role.
- The registry dressups of a Project Curve hold the rapid-order dressup, and the cut-run spans hold
  no barrier. Without a barrier the dressup can move a chain to another pass. So the runner adds one
  barrier before each later pass (`the_ladder_keeps_its_order_and_its_rapids_after_the_dressups_p33`).

## 7. The stated cut with no ladder: not built (a departure from the brief)

The brief asks that the stated cut is the full depth when the op has no ladder. That is not built.

- The config holds no full depth. `depth` is an offset from the model surface, and the material top
  is not in the config.
- `top_z - lowest tip Z` is the full depth only where the op top Z is the local material top. The
  operator job states the top (`top_z` manual 16.5); `planning/fixtures/rivmap100` "Project Curve"
  has an automatic top (the stock top) over a relief that an earlier op cut, so the value would be
  a false Exceeds there.
- A rule that would hold: with a STATED op top Z and a stored result, the body depth is at least
  `top_z - lowest tip Z of the result` (`BodyDepth::AtLeast`, a row that can prove Exceeds and never
  Within). It needs a new plan fact through `SuggestSlots`, `SuggestContext` and `PlanFacts`, and a
  lowest-Z value held with the result (the result stats hold no Z today; a walk of the moves on each
  read is too slow for the GUI). The lead decides.

On the operator job today: the river pass states no depth before a simulation (section 3). With
`depth_per_pass` set, the stated depth is the depth per pass, and the force budget row reads it
(tested on the fixture of section 5; NOT run on the operator file).
