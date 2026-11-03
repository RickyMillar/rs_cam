# Root cause 04: op 3 "Pin drill, flip pins" and op 4 "Drill light holes"

Scope: read-only analysis, 2026-11-03. Tool 0 (T1): 6 mm 2F flat end mill,
`cutting_length` 25, `shank_diameter` 6, `stickout` 30, no `non_centre_cutting`
(the engine reads "centre cutting"). The project has no `head` table, so the
head is `MachineHead::unmodelled()`: one hard 25 mm band at the holder face.

Runs (scratch copy `scratch_op3op4drill/w21.toml`, binary built 09:03 after
HEAD 08:41, `rough-score --no-sim`):

| Run | Op | Cutting mm | Descents (retract runs) | Read |
|---|---|---|---|---|
| `rs_tp3.json` | 3, peck 6 | 187.5 (5 holes x 37.5) | 30 (6 per hole) | R 30.5 to Z -4.5 |
| `rs_tp3_peck15.json` | 3, `--set 3.peck_depth=15` | 180.0 (5 x 36) | 15 (3 per hole) | the Suggest peck |
| `rs_tp4.json` | 4, peck 6 | 588.0 (42 x 14) | 126 (3 per hole) | R 21.5 to Z 8.5 |
| `rs_tp4_peck15.json` | 4, `--set 4.peck_depth=15` | 546.0 (42 x 13) | 42 (1 per hole) | the Suggest peck |

Two facts from the runs correct the review inputs:

1. The cycle starts at the R-plane, not at the hole top
   (`ops/drill.rs:205-232` `fed_descents`; `compute/config.rs:169-171`
   `effective_safe_z` = max(retract_z, top + 5)). Op 3 with peck 6 cuts
   1, 6, 6, 6, 6, 5 mm of material per descent. Op 4 cuts 1, 6, 1 mm.
2. Suggest does not keep the 6 mm peck. It writes 15 mm (see (c)).

---

## (a) Depth against the flute and the stick-out: no check fires on op 3

**Symptom.** Op 3 plunges 30 mm. The flute is 25 mm. The holder face is 30 mm
above the tip, so at the hole bottom (Z -4.5) the face is at Z 25.5, the
stock top. No warning, no diagnostic and no collision count shows this
(`rs_tp3.json`: `collision_count` 0).

**Code paths.** Each check that reads `cutting_length` or `stickout`, and why
it is silent on op 3:

| Check | File:line | Why it is silent |
|---|---|---|
| Static `geom.dpp_exceeds_cutting_length` | `diagnostics/adapters/from_static_checks.rs:474-480` | Reads `op.depth_per_pass()`. Drill and AlignmentPinDrill return `None` (`compute/catalog.rs:845-847`; Suggest record: "AlignmentPinDrill has no depth_per_pass"). The check also compares one pass, not the total depth. |
| Suggest pass 5 `clamp_dpp_to_cutting_length` | `feeds/suggest/invariants.rs:270, 561-575` | Same: no `depth_per_pass` on a drill op. |
| Calculator flute guard (0.8 x flute) | `feeds/mod.rs:2110-2122` | Reads `ap`. A drill sends no axial hint (`feeds/suggest/apply.rs:1196-1203`, registry `FeedsHints::NONE`), so `ap` = `MIN_AP` 0.05 mm (Suggest record `axial_depth_mm` 0.05). The hole depth never enters. |
| Drill gates (chip welding, peck adequacy, plunge feed) | `tool_load/drill_gates.rs:151-176`; `ops/drill_metrics.rs:190-202` | `DrillOp` (`ops/drill_op.rs:133-149`) has no flute length and no stick-out. Peck adequacy reads peck/D = 1.0 against 5.0 (oak). Chip welding credits each full retract and reads the per-peck ratio, so a hole deeper than the flute still scores "cleared". |
| Depth gate (sim) | `tool_load/depth.rs:128-132` | A drill cycle is `Unmodeled(NotApplicableForOp)`. |
| Sim axial engagement | `dexel_stock/simulation.rs:1346-1373` | A plunge sample puts the descent in `plunge_descent_mm` and sets axial engagement to 0. `axial_doc_fraction` is also clamped to 1.0, so it cannot show "past the flute". |
| Holder/shank vs mesh | `stock/collision.rs:422, 441, 331`; `compute/collision_check.rs:104-133` | Three independent misses: (1) model 5 is `block_outline.dxf`, a 2D model with no mesh, so the mesh half does not run; (2) a band whose radius is not larger than the cutter radius is skipped (`seg_radius <= cutter_radius + 1e-6`), and the shank is 6 mm on a 6 mm cutter; (3) a strike needs a penetration > 0.01 mm, and the holder face meets the stock top at exactly 0. The pins are also outside the block outline, where no mesh exists. |
| H1 obstacle view vs stock box | `compute/obstacle.rs:672-694, 699-721` | Only `AssemblySegment::Shoe` bands test against the fresh stock box. The holder band tests against obstacles (the clamps) and the shelf floor only. The unmodelled head has no shelf (`machine/head.rs:267-277`). |
| Pin drill tool template | `session/pins.rs:616-619` | The only code that relates the hole depth to the tool: `cutting_length >= stock + penetration`, `stickout >= cutting_length`. It runs only when the operator adds a new pin drill tool. It permits `stickout == depth`, the op 3 state. |

**Root cause.** The engine has no check that compares a drill hole depth with
the flute length or with the stick-out. Every flute and holder check reads a
per-pass depth, a mesh, or the shoe, and a drill op has none of these.

**Evidence.** The table above; `rs_tp3.json` (`collision_count` 0, 30
descents); `rg -n "cutting_length|stickout"` in `ops/`, `tool_load/` and
`compute/execute/drilling.rs` finds no comparison with a hole depth.

**Fix options.**

1. Add one static check for both drill families: deepest hole depth (from
   `build_drill_op_for_config`, `compute/execute/drilling.rs:60-150`) against
   `cutting_length` (warn: chips cannot leave the flute) and against
   `stickout` (critical when depth >= stickout - clearance: the holder face
   reaches the stock top). Put it beside `depth_checks` in
   `diagnostics/adapters/from_static_checks.rs` with a new id in
   `diagnostics/ids.rs`. Blast radius: `depth_checks(` has 2 hits in 1 file;
   a new id needs a presentation row (`diagnostics/presentation.rs`).
2. Add the holder band to the H1 stock-box test (`compute/obstacle.rs:682`
   accepts `Shoe` only). This catches every op, not only drills. Blast
   radius: `.for_tool(` has 19 hits in 10 files; every op with a short
   stick-out under a holder face below the stock top changes its count.
   Use a clearance, not 0.01 mm penetration, or the op 3 equality still
   passes.
3. Change `pin_drill_tool_template` (`session/pins.rs:618-619`) to ask for
   `stickout > depth + clearance` and `cutting_length > stock height`.
   Blast radius: 4 hits in 2 files (`rs_cam_viz/src/controller/events/pins.rs`).

**Sentries.** `drill_op_step3`, `drill_metrics_pr2`, `drill_peck_depth_g_wanaka_peck`,
`pin_holes_are_drilled_by_size_r1`, `pindrill_emission_frame_g_pindrill`,
`the_machine_head_guards_the_checks_h1`, `a_failed_collision_check_is_not_a_clean_one_core_g_colfail`,
`tier_map_flute_reach` (the one "body above the flutes" test, tiered finish
only); viz `holder_clearance_scope_g_holderscope`,
`holder_clearance_staleness_g_holderstale`.

**Confidence.** High.

---

## (b) Where the 30 mm pin depth comes from

**Symptom.** The pin hole is 30 mm: the stock 25.5 plus 4.5 into the
spoilboard.

**Code path.** `compute/execute/drilling.rs:478-479`:
`depth = (stock_bbox.max.z - stock_bbox.min.z) + cfg.spoilboard_penetration`.
The same expression is at `drilling.rs:125-128` (the `DrillOp` view) and
`feeds/suggest/apply.rs:1097-1103` (the Suggest peck cap).
`spoilboard_penetration` is an operator field, range 0.5-20
(`compute/operation_configs.rs`, `AlignmentPinDrillConfig`).

**Root cause.** The engine does not derive the pin depth from the tool or
from the flip. The 4.5 comes from the planner script:
`planning/rivmap_block_job/derive_wanaka21.py:594-596` writes
`spoilboard_penetration = cam.pin_depth - T1` with the comment "Depth =
block.json cam.pin_depth 30.0 = the flat_6 reach". The older script states
the rule: `derive.py:598` "bore = the stick-out (testpiece rule: 30 =
reach)". The planner equates "reach" with the stick-out, not with the flute.

**Evidence.** The operator TOML lines 843-851 (`spoilboard_penetration =
4.5`); `nz_wanaka_21.toml:805`; the run `rs_tp3.json` (bottom Z -4.5).

**Fix options.** In the planner: reach = `cutting_length` - margin, or a
separate pin tool. In the engine: fix option (a)1 makes the mismatch
visible. A derived default (penetration from the tool) is not advised: the
penetration is a fixture choice.

**Sentries.** `pin_holes_are_drilled_by_size_r1`, `a_pin_set_edit_stales_only_the_pin_drill_p4`,
`pindrill_emission_frame_g_pindrill`, the DR-PIN unit test
`alignment_pin_drill_peck_is_clamped_like_drill` (`feeds/suggest/tests.rs:871`).

**Confidence.** High.

---

## (c) The peck 6 mm, the dwell 0.5 s, and the Suggest feed 1835 at 14000

### The peck

**Symptom.** Both ops carry `peck_depth = 6.0`. `compare.md` says "Suggest
keeps the operator's peck and depth".

**Code path.** The 6.0 comes from the planner: op 3 `peck_depth =
dia["flat_6"]` (`derive_wanaka21.py:595`); op 4 copies the testpiece drill op
(`derive_wanaka21.py:400, 599-601`). Suggest then overwrites it:
`suggest_for_operation` applies `ApplyScope::Both` (`feeds/suggest.rs:957-969`),
and `apply` writes the drill defaults under a geometry scope
(`feeds/suggest/apply.rs:900-902`). `apply_drill_defaults`
(`apply.rs:1074-1106`) writes `drill_default_peck_depth_mm` = half of the
per-peck maximum (`material/mod.rs:1481-1485`); white oak (Janka 1360) is in
the 5 x D band (`material/mod.rs:1105-1113`), so 15 mm. `peck_for_hole`
(`apply.rs:1123-1135`) caps it at 0.75 x depth:

- Op 3: depth 25.5 + 4.5 = 30, cap 22.5, so Suggest writes **15**.
- Op 4: the floor is `from = "z"`. `Floor::depth_below_top` returns `None`
  for a Z floor (`compute/floor.rs:165-177`), so no cap: Suggest writes
  **15** against an 8 mm hole. From R 21.5 that is one 13 mm descent
  (`rs_tp4_peck15.json`: 42 descents for 42 holes), a single-shot cycle
  with a `Peck` label. This is the DR-PIN defect again, for the Drill
  family under a Z floor.

**Root cause.** The suggest CLI prints five fields only
(`crates/rs_cam_cli/src/suggest.rs:29-37`, no `peck_depth`, no
`dwell_time`), and `compare_calc.py:326-328` asserts "Suggest never writes
... the peck or the dwell". Both are wrong for the peck. In the engine,
`peck_for_hole` gets no depth for a Drill whose floor is not `Floor::Top`.

**Fix options.**

1. Print `peck_depth` and `dwell_time` in `op_values`
   (`rs_cam_cli/src/suggest.rs:29`). Blast radius: the CLI only; the review
   JSON gains two keys.
2. Give `apply_drill_defaults` the per-hole depth for every floor (the
   deepest of `hole_floors`, as `drill_depth_bases` does at
   `drilling.rs:162-200`), so the 0.75 cap applies. Blast radius:
   `apply_drill_defaults` 22 hits in 7 files; `peck_for_hole` 5 hits in 2.
   The FM1 golden matrix (`feeds_matrix_instrument_fm1`) can move.
3. Cap the Suggest peck at `cutting_length` minus a margin for a hole
   deeper than the flute. Same blast radius as option 2.
4. Root the peck at the hole top, not the R-plane. Blast radius:
   `fed_descents(` 8 hits in 4 files, and the R-2 rule says one schedule
   for the emitter and the metrics. This changes every drill toolpath;
   do not do it without a ruling.

**Sentries.** `drill_peck_depth_g_wanaka_peck`, `a_peck_ladder_is_never_split_into_a_rapid_into_stock_g_pecksplit`,
`drill_fed_descents_motion`, `drill_cycle_time_integration_g_drilltime`,
unit tests `drill_peck_depth_scales_with_diameter_and_material`,
`alignment_pin_drill_peck_is_clamped_like_drill`,
`a_suggested_peck_respects_the_schema_floor_b14` (`feeds/suggest/tests.rs:782, 871, 2601`),
`feeds_matrix_instrument_fm1`.

**Confidence.** High for the Suggest value 15 (code path and the
`--set` runs). The suggest JSON does not show it directly.

### The dwell

**Symptom.** Op 4 carries `dwell_time = 0.5`. The reviews flag 117-150 rev
of rubbing.

**Code path.** The dwell is read only by `DrillCycleType::Dwell`
(`compute/operation_configs.rs:216`). `PhaseDrill::dwell_s` returns `None`
for every other cycle (`gcode/mod.rs:515-520`), and the emitter adds `G4`
only when `dwell_s` is `Some` (`gcode/program_builder.rs:590, 623-630`).
Op 4 is `cycle = "peck"`. The GUI hides the field for a non-Dwell cycle
(`rs_cam_viz/src/ui/properties/operations/drill.rs:138-140`).

**Root cause.** The 0.5 s is an inert field: the `DrillConfig::default`
value (`operation_configs.rs:435`) that the testpiece file and the planner
copy. A peck cycle emits no dwell. The risk in the review does not exist in
the G-code. A native G83 is the one exception: grblHAL dwells 0.25 s per
peck (`program_builder.rs:775-780`), but `native_drill_cycles` is off by
default and the project does not set it.

**Fix options.** Drop the key in the planner, or have `save` skip
`dwell_time` for a non-Dwell cycle. Blast radius: project files and the
`compute/operation_configs.rs:4115` fixture. `dwell_s(` has 3 hits in 2 files.

**Sentries.** `drill_cycle_time_integration_g_drilltime`; the G7 emitter
tests `native_drill_cycles_fall_back_on_an_unknown_shape_g7` and
`native_drill_cycles_keep_the_obstacle_lead_in_k3` (`gcode/emitter.rs:906, 944`).
No test pins "a peck cycle emits no G4": add one if the field stays.

**Confidence.** High (code). Not confirmed on emitted G-code: `project
--emit-gcode` always simulates, which this brief does not allow.

### The feed 1835 at 14000

**Symptom.** Suggest raises op 3 from 736 at 10610 (fz 0.035) to 1835 at
14000 (fz 0.0656 axial).

**Code path.** The G6 drill claim (`feeds/extrapolation/drill.rs:1-28,
84-99`): the Amana Spektra chart prints Ramp Down = feed / Z, so axial chip
= side chip / Z. The 6 mm 2F Wood row gives 0.0635 at 18000; the hardness
transfer to white oak gives x1.0326 = 0.0656
(`tests/literature_matrix/cells.toml:4826-4845`). Step 2c clamps the RPM to
the drill tier 8000-14000 for D <= 6 (`feeds/mod.rs:1364-1398`), and the
chip per tooth holds: 14000 x 2 x 0.0656 = 1836, floored to 1835. The
L/D 5 `LongToolDerate` (factor 0.88) does not change the feed by ruling R4
(`feeds/mod.rs:2236-2255`). 1835 / 6 = 306 mm/min per mm, inside the plunge
envelope 50-580, so the plunge gate passes.

**Root cause.** Suggest reads one printed point at "depth of cut 1 x D"
and has no input for the hole depth, the depth/flute ratio, or the machine
class. The reviewer derates the same 0.0635 to 30-50 % for a 30 mm hole on
a hobby machine (judgement). This is a missing input, not an arithmetic
error.

**Fix options.** Give the drill path an axial hint (the deepest hole) and
apply the chart's own depth rule ("2 x D -25 %, 3 x D -50 %",
`sources.toml:203`) to the drill feed. Blast radius: `FeedsHints` per op in
`compute/catalog/registry.rs`, the test `operation_feeds_hints_is_an_explicit_per_op_decision`
(`feeds/suggest/tests.rs:2544`), the FM1 matrix and both drill lit cells.

**Sentries.** `a_flat_end_plunge_is_the_side_chip_over_z_g6`,
`a_plunge_is_a_named_fraction_of_the_side_feed_g10`, `_litmatrix_drill_rpm_ceiling`,
`_litmatrix_drill_rpm_diameter_tier`, `literature_matrix`, `feeds_matrix_instrument_fm1`.

**Confidence.** High for the path; the size of a depth derate is a ruling.

---

## (d) An end mill used as a drill

**Symptom.** `sources.toml` names the cell
`flat_6mm_drill_oak_using_endmill_unadvised`.

**Code path.** `crates/rs_cam_core/tests/literature_matrix/cells.toml:4800-4920`.
Inputs: 6 mm 2F flat, flute 22, stick-out 25, drill depth 18, white oak.
Expected: the same 0.0643-0.0669 band as the normal-use twin
`flat_6mm_drill_oak` (`cells.toml:1689`); `expected_behaviour = unadvised`,
`expected_derate = 0.7`, warning pattern `endmill|center.?cut|drill|peck|plunge`.
The runner (`tests/literature_matrix/runner.rs:211-255`) treats a missing
warning as informational (Cosmetic), leaves the derate as a phase-2 TODO,
and caps the verdict at Minor.

**Root cause.** In the engine an end mill in a drill op is not a
special case: it is the only backed case. The single G6 rule reads a flat
end mill row (`DRILL_RULES`, `extrapolation/drill.rs:84-99`); every other
drill cell refuses. The "unadvised" label lives only in the test matrix.
The cell has depth 18 < flute 22 < stick-out 25, so it never tests depth
against the flute or the stick-out. No engine warning is required, and the
0.7 derate is never applied.

**Implication.** The op 3 geometry (depth 30 >= stick-out 30 > flute 25) has
no matrix cell. The engine output for an end mill drill is the chart
point at any depth.

**Fix options.** Add a cell with depth > flute and depth >= stick-out, with
an anti-pattern `drill_depth > flute_length_mm` and an invariant on
`stickout`. The shim already passes `flute_length_mm` and `stickout_mm`
(`tests/literature_matrix/shim.rs:237-240`) and `drill_depth_mm`
(`shim.rs:317-328`), but the expression bindings need `drill_depth` and
`flute_length` (`shim.rs:573` binds `stickout` only). Blast radius: the
test matrix only.

**Sentries.** `literature_matrix`, `literature_parity`,
`_litmatrix_drill_rpm_ceiling`.

**Confidence.** High.

---

## Summary of root causes

| Item | Root cause | File:line |
|---|---|---|
| (a) no depth vs flute / stick-out check | Checks read `depth_per_pass` (None on drills), a mesh (none on a DXF), or the shoe band; shank radius = cutter radius is skipped; 0 mm holder clearance is not a strike | `from_static_checks.rs:474-480`; `stock/collision.rs:441, 331`; `compute/obstacle.rs:682`; `drill_op.rs:133-149` |
| (b) 30 mm depth | Not derived: stock + operator `spoilboard_penetration`; planner sets reach = stick-out | `compute/execute/drilling.rs:478-479`; `derive_wanaka21.py:594-596`; `derive.py:598` |
| (c) peck | Suggest writes 15, not 6; op 4 Z floor gives no depth cap; the CLI hides the peck | `feeds/suggest/apply.rs:900-902, 1074-1135`; `compute/floor.rs:165-177`; `rs_cam_cli/src/suggest.rs:29-37` |
| (c) dwell | Inert on a peck cycle: no G4 emitted | `gcode/mod.rs:515-520`; `gcode/program_builder.rs:623-630` |
| (c) feed 1835 | G6 chart point x hardness at the 14000 drill cap; no hole-depth input | `feeds/extrapolation/drill.rs:23-28`; `feeds/mod.rs:1381-1398` |
| (d) end mill as drill | The only backed drill case; "unadvised" is a test label with no engine effect | `cells.toml:4800-4920`; `runner.rs:211-255` |
