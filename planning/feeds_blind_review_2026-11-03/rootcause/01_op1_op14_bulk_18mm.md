# Root cause 01: ops 1 and 14 (and 0 / 12), the 18 mm 2F non-centre-cutting bulk bit

Scope: read-only analysis at HEAD 40cf411a7 (binary `target/release/rs_cam_cli` built 2026-11-03 09:03, after HEAD). No source edit, no cargo run.

Evidence files in `scratch_op1_bulk18/`:

- `wanaka21_op1.toml`: a copy of `planning/rivmap_block_job/nz_wanaka_21_operator.toml`.
- `op1_*.json` / `op1_*.err`: `rough-score --toolpath 1 --no-sim` runs (base and `--set` variants).
- `check_numbers.py`: asserts every number in this file (`python3 -I check_numbers.py`; all assertions hold).

Inputs that Suggest reads for this tool:

- Tool: D 18, Z 2, `cutting_length` 20, stick-out 25, core 8, protrusion 1.
- Machine: `rigidity.adaptive_doc_factor` 2.0, `aggressiveness` 0.85, VFD 1.5 kW at 24000 rpm, no `max_cutting_feed_mm_min`.
- Material: White Oak.

---

## (a) depth_per_pass 14.047 mm (operator 9.0 on op 1, 4.1667 on op 14)

**Symptom.** `suggest_operator.jsonl` ships `depth_per_pass` 14.047 for ops 1 and 14. The calculator record has `axial_depth_mm` 16.0 and `DocExceedsFlute { requested: 36.0, capped: 16.0 }`. The rationale says "Depth per pass 16.000 -> 14.047 mm: aggressiveness 0.85".

**Code path.**

1. `crates/rs_cam_cli/src/suggest.rs:117` calls `ProjectSession::cutter_op_profile` (`session/mod.rs:2323`). That calls `CutterOpProfile::for_combo` (`feeds/profile.rs:153`), then `suggest_for_operation` (`feeds/suggest.rs:941`). That calls `apply(.., ApplyScope::Both, ..)` (`suggest.rs:957`).
2. The 3D Rough registry row declares no feeds hint: `feeds_hints: FeedsHints::NONE` (`compute/catalog/registry.rs:824`). So `operation_feeds_hints` (`feeds/suggest/apply.rs:1196`) gives `axial_depth_mm = None` (`suggest.rs:996, 1016`). The operator's 9.0 never reaches the calculator.
3. Calculator Step 3 (`feeds/mod.rs:2076-2077`) uses `default_engagement`, Adaptive arm. The base factor is `ADAPTIVE_FLAT_AP` 1.20 (`mod.rs:3224`), and the hardwood scale lowers it further. Then `ap_factor = ap_factor.max(adaptive_doc_factor * 1.5 / 1.5)` (`mod.rs:3433-3436`). With 2.0 the factor is 2.0, so ap = 2.0 x 18 = **36 mm**. The machine rigidity value is a FLOOR here, not a cap. The commit 1dc64150a ("softer rigidity factors") keeps 2.0.
4. Step 4 flute guard (`mod.rs:2110-2121`): `flute_length x FLUTE_GUARD_FACTOR` = 20 x 0.8 = **16 mm**. `FLUTE_GUARD_FACTOR` is `unsourced` (`mod.rs:1539`).
5. `apply_feeds_subset` (`feeds/suggest/apply.rs:160-183`) writes the result's 16.0 into `depth_per_pass` under `Both`. The depth snap at `apply.rs:178-181` does nothing, because `Adaptive3dConfig` declares `total_depth: none` (`compute/operation_configs.rs:3770`).
6. `run_invariants` (`feeds/suggest/invariants.rs:186`) runs these passes:
   - Pass 0 `pick_axial_envelope` (`suggest/axial_envelope.rs:263-273`) only LOWERS a depth. Its vendor bound is `ap_max_factor 1.0 x 18` = 18 mm (`cutter_constraints.rs:346`), so 16 stands.
   - Pass 4 `clamp_dpp_to_rigidity` (`invariants.rs:475`) caps at `adaptive_doc_factor x D` = 36 mm, so it does not act.
   - Pass 5 caps at the 20 mm `cutting_length` (`invariants.rs:561`), so it does not act.
   - Pass 6 deflection back-off (`invariants.rs:599`) does not act (no warning).
7. Pass 6b `apply_aggressiveness` (`suggest/aggressiveness.rs:146`) bisects one common scale s on depth and stepover, to 0.85 of the load at 16 x 3.6. The result is s = 0.878: 16 -> 14.047 and 3.6 -> 3.16 (`aggressiveness.rs:220-250`). The depth snap (`aggressiveness.rs:232`) needs `total_depth()`, which is `None` again, so 14.047 is a raw number. The rationale text "rounds to a step the generator can cut" is false for this operation.

**Root cause.**

- 3D Rough gives the calculator no depth hint, so Suggest starts from the generic rule: rigidity floor 2 x D, then cut to 0.8 x flute = 16 mm.
- `ApplyScope::Both` then overwrites the operator's depth. The sum is therefore a function of `adaptive_doc_factor`, `cutting_length` and the dial only.
- No input reads the job's levels: `heights.bottom_z`, the model, or the 9 mm cavity floor. `model_bbox` is read only for the stepover back-off and the strategy class (`invariants.rs:417`, `adaptive_entry.rs:77`).
- `Adaptive3dConfig::total_depth` is `none`, so no snap step can see the level structure.

For op 1 (T1 25.5 to bottom 11), `adaptive3d/path.rs:583 step_levels` with 14.047 gives a level at 11.453 and a 0.45 mm skin at 11.0. The 9 mm cavity level is gone. For op 14 the relief becomes one deep level instead of the planned 4.17 mm steps.

**Evidence.** `suggest_operator.jsonl` index 1 and 14: `"DocExceedsFlute { requested: 36.0, capped: 16.0 }"`, rationale `from_value 16.0 to_value 14.047`, stepover `3.6 -> 3.16`. `check_numbers.py` asserts 36 = 2 x 18, 16 = 0.8 x 20, and 14.047 / 16 = 3.16 / 3.6.

**Fix options and blast radius.**

1. Give `Adaptive3d` an axial hint, the operator's `depth_per_pass`, in its registry row (`registry.rs:824`). Then the calculator sizes at the operator's depth, and Suggest only lowers it. `feeds_hints` has 54 hits in 10 files. The table test `operation_feeds_hints_is_an_explicit_per_op_decision` (`feeds/suggest/tests.rs:2544`) pins `Adaptive3d => (None, None, None)` and must change. The FM1 golden matrix moves; re-bless with `RS_CAM_UPDATE_FEEDS_MATRIX=1`.
2. Make the rigidity term at `mod.rs:3433` a cap, not a floor, for Adaptive ap (`min` in place of `max`). `adaptive_doc_factor` has 41 hits in 11 files. This moves every adaptive cell of FM1.
3. Give `Adaptive3dConfig` a `total_depth` from `heights` (top_z - bottom_z), so `realised_step_down` snaps at `apply.rs:178` and `aggressiveness.rs:232`. `realised_step_down(` has 12 hits in 5 files. This does not stop the raise (16 > 9), so it is a supplement to fix 1, not a replacement.
4. Do not raise an operator depth under `Both` for a 3D Rough. That is, pass 0 and the apply write become min(operator, calculator). This has the smallest blast radius: `apply.rs` only.

**Sentries.**

- `feeds_matrix_instrument_fm1` (golden), `the_dial_holds_the_load_and_never_cuts_the_feed_fm7` and `a_speeds_only_apply_ships_the_feed_for_the_depth_it_keeps_g_speedsapply`.
- `a_published_power_is_at_the_depth_that_cuts_g_s2`, `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` and `--lib feeds::suggest` (`operation_feeds_hints_is_an_explicit_per_op_decision`).
- `wanaka_suggest_integration` is slow: ask first.

**Confidence.** High. Every step is in the shipped JSON and the code above.

---

## (b) 16219 rpm, feed 5999, and the pre-sim power 0.53 kW against the 1.18 kW sim peak

**Symptom.** Suggest ships 16219 rpm and 5999 mm/min (`RpmLoweredForFeedCeiling { rpm_from: 18000, rpm_to: 16219, feed_ceiling_mm_min: 6000 }`). `shipped_power` is 0.531 of 1.014 kW. The 2026-11-02 sim (`planning/rivmap_block_job.md:722`) measured 1.18 kW at 9 x 3.6.

**Code path.**

1. Step 1/2: RPM = the row's `rpm_nominal` 18000 (`spindle_strategy = "match_chart"`). The chip is 0.18496 (see (c)). The feed is 18000 x 0.18496 x 2 = 6659 mm/min.
2. Step 7 (`feeds/mod.rs:2603-2672`): the machine sets no `max_cutting_feed_mm_min`, so the ceiling is `DEFAULT_CUTTING_FEED_CAP_MM_MIN` = 6000 (`machine/mod.rs:628, 657`), not the 15000 travel rate. Ruling R4 Q10 holds the chip and lowers the RPM: floor(6000 / 0.36992) = **16219**. On the constant-torque VFD that also lowers the available power to 1.5 x 16219 / 24000 = **1.014 kW**. `fits_power` (`mod.rs:2618-2630`) checks the calculator's own ap/ae (16 x 3.6 = 0.669 kW), so the descent does not stop. The funnel rounds the feed down to 5999 (`apply.rs:139`).
3. The pre-sim power is `power_at_operating_point` (`feeds/operating_point.rs:205`) at the operation's `depth_per_pass` and `stepover`, with the nominal immersion. At 14.047 x 3.16 that is 0.531 kW; at 9 x 3.6 it is 0.376 kW. This is the same two-term model as the gate (`tool_load/power.rs` module doc).
4. The sim gate `tool_load::power::evaluate` (`tool_load/power.rs:312`) reads each sample's measured `arc_engagement_radians` and `axial_doc_mm` (`sample_power_kw`, `power.rs:257-295`). It excludes configured-entry samples into the entry advisory (`power.rs:460-466`). So the 1.18 kW peak is a **steady-state cut sample, not a ramp**. `check_numbers.py` shows that 1.18 kW at ap 9 needs ae about 14.2 mm (arc 125 deg, against 53 deg nominal). As a full slot it needs ap 6.8 mm. The adaptive path has near-slot moments that the nominal ae does not describe, for example the cut after a ramp and region openings. The sim JSON of 2026-11-02 is not in the repo, so the sample index and locality are not recovered here. A sim is needed: read `PowerVerdict::Exceeds.evidence` locality.

**Feedback paths that exist.**

- **Export refusal:** a power `Exceeds` has `BoundSource::MachinePowerCurve`, and `gates_export()` is true for every source except `RigidityRuleOfThumb` (`tool_load/verdict.rs:1142`). So `enforce_exceedance_policy` (`gcode/mod.rs:831`) refuses the export unless the operator accepts the exceedance with `accept_exceeded_tool_load`.
- **Sim feed modulation:** the session passes `PowerLimitInputs` (`session/compute.rs:2171-2226`). The constrained-max solver caps the feed per move at the power curve (`dressup/feed_modulation.rs:564-612`). This writes modulated feeds into the export IR. It does not change the operation's `feed_rate`, and Suggest does not read it. The data does not show whether the 2026-11-02 sim ran with modulation on.
- **Suggest:** no path. `feeds/profile.rs:17-18, 86` states that the profile "must not own the post-sim `ToolpathLoadVerdict`". No pass of `run_invariants` reads a verdict.

**Root cause.**

- The pre-sim power is evaluated only at the nominal engagement. No margin for the engagement excursions of an adaptive path is in it, so Suggest calls 52 % safe.
- The RPM descent at the unsourced 6000 mm/min default cap holds the chip and gives up power: it lowers the available power to 1.01 kW. It does not consider the opposite move (24000 rpm, fz 0.125, 1.5 kW available).
- No path returns the sim power peak to Suggest. The only consequences are an export refusal and the per-move feed cap of the modulator.

**Fix options and blast radius.**

1. Set `max_cutting_feed_mm_min` in the machine profile if the Shapeoko can cut faster. That is a project change, not code. `DEFAULT_CUTTING_FEED_CAP_MM_MIN` itself has 14 hits in 4 files, and `cutting_feed_ceiling_mm_min()` has 24 hits in 11 files.
2. In Step 7, compare the power headroom of "lower RPM, same chip" against "same RPM, smaller chip" when the ceiling binds. This touches `mod.rs:2603-2672` and the sentry `rpm_follows_the_feed_ceiling_fm8`.
3. Add an adaptive excursion factor to the pre-sim power check (for example, the arc at the planner's engagement cap `trochoid_cap_mult` 1.6, or a slot at the depth). `power_at_operating_point(` has 20 hits in 10 files. This changes `a_published_power_is_at_the_depth_that_cuts_g_s2`, `power_ceiling_parity_f2` and `suggest_power_ceiling_after_pass9_g_suggest_powerstale`.
4. A sim-backed Suggest arm: read `PowerVerdict::Exceeds { peak_kw, available_kw }` and scale the depth, stepover or feed. This is new wiring through `session` to `feeds`. It conflicts with the profile.rs boundary, so it needs an operator ruling.

**Sentries.**

- `rpm_follows_the_feed_ceiling_fm8`, `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` and `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`.
- `the_power_ladder_pulls_the_right_lever_g_ladder` and `power_ceiling_parity_f2`.
- For the gate: `gate_population_vacuity_xvac` and `a_weak_bound_cannot_refuse_an_export_g_s4weak`.

**Confidence.**

- High for the RPM, feed and pre-sim figures. `check_numbers.py` reproduces 0.669 and 0.531 kW exactly.
- Medium for the cause of the 1.18 kW peak. The engagement needed is derived; the sample is not recovered.

---

## (c) fz 0.185 mm/tooth from the G1 size claim, no chip-thinning limit at 0.2 D

**Symptom.** `chipload_point_mm` 0.18496, `chipload_bounds_mm` null, support "extrapolated (G1 size): x1.24 from the 12.7 mm row".

**Code path.**

1. Row `amana-flat-hardwood-pocket-12700-2f-spektra`: one printed CPT 0.0057 in = 0.1448 mm at 12.7 mm and 18000 rpm, `ap_max_factor` 1.0 (`data/vendor_lut/observations/amana_flat_end.json:2706`). The chart is a Spektra **spiral plunge** (centre-cutting) series. The project tool is a non-centre-cutting bulk bit. The series match reads only the cut direction (`feeds/series.rs`, `ToolSeries::for_cut_direction`, `suggest.rs:1000`), so nothing tells the two tools apart.
2. G1 size, form C (`feeds/extrapolation/size.rs`, rule `RULE_C`): 18 / 12.7 = 1.417 is past the printed series span by more than one step, so the claim is form C (d/d_row)^0.61 = x1.237 (`vendor_lookup.rs:537`).
3. G2 hardness (`feeds/extrapolation/hardness.rs:283`): (1450 / 1360)^0.5 = x1.033 (`vendor_lookup.rs:573`; WhiteOak Janka 1360, `material/mod.rs:153`; the row's proxy is 1450). Product: 0.1448 x 1.237 x 1.033 = **0.18496** (asserted).
4. A2: one printed value is a point. No band is derived, and `chipload_bounds` is `None`. The gate is hard only above the point.
5. Step 5 (`feeds/mod.rs:2164-2209`): chip thinning is "MEASURED, NOT APPLIED" (G-CHIPTHIN-HALFFIX, operator ruling 2026-08-19). `rctf` is computed and not multiplied in. The comment says why: no wood chart prints a radial condition. So `ae` does not enter the feed. The depth tier is 1.0 because ap 16 < 1 x D (`mod.rs:2226-2232`).

**Root cause.**

- The feed is the extrapolated printed chip as advance per tooth. No rule lowers it for the radial engagement, the extrapolation, or the tool type. The ruling removed the radial term in both directions, and A2 caps only from above, at the same point.
- At 0.2 D the real chip is hex = 0.18496 x 2 sqrt(r(1 - r)) = 0.141 mm at ae 3.16. That is about the printed 12.7 mm chip 0.145, so by the chart the chip is not unsafe.
- The reviewers' lower fz (0.08-0.09) is a power and depth argument (see (b)), not a chip-thinning argument. Suggest has no reason to lower fz, because its nominal power check passes.

**Fix options and blast radius.**

1. A size-claim cap: an extrapolated (form C) chip above the anchor row's printed point is capped at that point, unless a source says otherwise. This touches `extrapolation/size.rs` and the consumers of `LookupResult::size_basis`. Sentries: `a_size_claim_states_its_rule_range_and_residual_g1`, `every_consumer_reads_one_claimed_band_g1`; FM1 re-bless.
2. A tool-series flag for non-centre-cutting / bulk (insert-style) bits, so the Spektra plunge row is not the default. This touches `series.rs` `SERIES_RULES` (`a_generic_tool_reads_the_standard_row_first_g_series`, `lut_resolver_census_a6`).
3. Do not re-introduce chip thinning without a source: the ruling G-CHIPTHIN-HALFFIX holds. `radial_chip_thinning_factor(` has 8 hits in 3 files.

**Confidence.** High for the arithmetic and the path. The fix choice is a ruling for the operator.

---

## (d) The 2 straight plunges that remain after G-CENTRECUT (c0f5d2567)

**Symptom.** `rough-score --no-sim` on op 1 gives 44 entry runs: 42 `entry_ramp` and 2 `entry_plunge` (`scratch_op1_bulk18/op1_base.json`, same as `compare.json`). The generation is not blocked. A `NonCentreCuttingRefusal` would block it (`OperationError::NonCentreCutting`, commit message).

**Code path.**

1. `adaptive3d/path.rs` `segments_to_toolpath_checked`: `effective_entry_style` turns Plunge into Ramp. The operator style is already `ramp`. Each entry calls `crate::dressup::emit_ramp` (`path.rs:2285-2299`) with `centre_limits = cutter.non_centre_cutting()`.
2. `dressup/entry_descent.rs` `emit_ramp` (`:522`) clamps the angle to atan(1/8) = 7.13 deg. Three branches still emit a straight `MoveIntent::EntryPlunge`:
   - (i) **Air column:** `rapid_to_entry_top` (`:434-479`) finds `material_top <= end.z` and feeds straight down to the target. This happens when the planner floor `rapid_floor_z` sits within `RAPID_DESCENT_BUFFER_MM` 0.5 (`path.rs:1753`) above the entry Z. The descent is then <= 0.5 mm and in air, by the conservative stock read. No refusal is computed, because there is no material.
   - (ii) **G-RAMPTERRAIN clip failure:** the plunge falls back with `plunge_refusal(safety, ramp_start_z, end)` (`:668-673`). It refuses only when the depth is over the protrusion + tol (`compute/tool_config.rs:374, 398`). A plunge <= 1 mm passes by design.
   - (iii) **Invalid angle:** this cannot happen under G-CENTRECUT (`:539-543`).
3. `rs_cam_cli/src/rough_score.rs:431-470` labels a run by the intent of its FIRST fed move. So a ramp entry that starts with a short straight air feed is counted as `entry_plunge`. The proof: with `--set 1.entry_clearance_mm=0.0`, all 44 runs become `entry_plunge`. Each ramp then starts with a 0.5 mm air feed (`op1_1_entry_clearance_mm_0_0.json`).

**Discrimination runs (all `--no-sim`, op 1).**

| Override | entry_plunge / entry_ramp | entry_moves | Reading |
|---|---|---|---|
| none | 2 / 42 | 2162 | base |
| `entry_clearance_mm=2.0` | 2 / 42 | 2162 | no nominal-top (off-grid `Rapid`) entries: their 1.5 mm air feed would vanish |
| `entry_clearance_mm=0.0` | 44 / 0 | 1982 | the label is the first move |
| `ramp_angle_deg=7.0` | 2 / 42 | 462 | ramp legs 2.3 x shorter; the count holds, so a terrain-clip failure is unlikely |
| `stock_to_leave_axial=0.3` | 2 / 42 | 2079 | count holds |
| `ramp_angle_deg=1.0` | 47 / 42 | 7153 | a different path (3562 rapids, 72 retracts); not used |

**Root cause.** The 2 runs most likely come from branch (i): entries where the planner's floor is at, or less than 0.5 mm above, the entry level. `emit_ramp` then feeds a <= 0.5 mm vertical move through what the planner reads as air. G-CENTRECUT allows it ("air is straight", `dressup/CLAUDE.md`). The refusal rule permits any straight plunge up to the 1 mm protrusion. So these are not a leak of the rule, but a sim must confirm that they land in air.

The real G-CENTRECUT gap is a different one: the commit says "3D Rough cut and link descents ... are held by the gate only, not planned to the limit" (`adaptive3d/CLAUDE.md`). Steep descents inside the cut path and the stay-down links are not entries. rough-score does not count them, and only the simulation walk (`RapidClearanceCheck`) and the export refusal catch them.

**Fix options and blast radius.**

1. rough-score: count an entry run by its deepest fed move into stock, not by its first intent, or report the plunge depth. This is CLI only (`rough_score.rs`, 1 file).
2. `rapid_to_entry_top` air branch: emit the air descent as a `Linking` feed (not `EntryPlunge`) when the column is open. This touches 4 call sites in 1 file. The plunge-rate stages read the intent, so check `plunge_guard_p3` and `boundary_reentry_plunge_rate_g_boundaryplunge`.
3. Plan the cut and link descents of 3D Rough to the limit angle (the open item of c0f5d2567).

**Sentries.**

- `a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut` and `--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut`.
- `adaptive3d_entry_stock_aware`, `entry_moves_stock_aware_g_rampterrain` and `a_rest_entry_does_not_peck_through_cleared_air_d55`.

**Confidence.**

- Medium that the 2 runs are <= 0.5 mm air feeds (branch i). The runs exclude off-grid entries and make a clip failure unlikely, but a move dump or a sim must confirm it.
- High that any such plunge is <= 1 mm into the conservative stock, because a deeper one refuses the generation.

---

## (e) Ops 0 and 12 (Face): stepover 5.487 and depth 0.25

**Symptom.** Stepover 14.4 -> 5.487 and depth 0.5 -> 0.25. The calculator record has `axial_depth_mm` 12.6 and `radial_width_mm` 6.3. `shipped_power` is 0.015 kW (1.4 % of the available power).

**Code path.**

1. The Face registry row: `feeds_family: Pocket, Roughing` and `feeds_hints: NONE` (`registry.rs:72-73, 86`). The calculator default for Pocket Roughing is ap 0.70 D = 12.6 and ae 0.35 D = 6.3 (`feeds/mod.rs:2994-2996`).
2. `apply.rs:159-183` writes ae 6.3 over the operator's 14.4. It writes ap 12.6, which the snap turns into `realised_step_down(0.5, 12.6)` = 0.5, because Face has `total_depth: depth`.
3. Pass 6b (`aggressiveness.rs:146`): target 0.85 x the load at 0.5 x 6.3, which gives s = 0.871. The depth asks for 0.435. The snap `realised_step_down(0.5, 0.435)` (`aggressiveness.rs:232`) gives 2 passes, so **0.25**. The stepover is 6.3 x 0.871 = **5.487**.

**Root cause.**

- The Face operation gives no stepover hint, so a pocket default of 0.35 D replaces a facing stepover of 0.8 D.
- The dial then scales the load as a share of that base, whatever its absolute size: 0.033 kW is 3 % of the spindle.
- On a 0.5 mm total depth, any s < 1 snaps the depth to two passes of 0.25. So the dial doubles the passes to remove 15 % of a load that is already negligible.

**Fix options and blast radius.**

1. Face hint: send the operator's stepover (and the face depth) as a radial hint (`registry.rs:86`; the table test at `suggest/tests.rs:2544` and FM1).
2. Skip the dial when the base load is far below the power and force limits (an absolute floor in `apply_aggressiveness`, 2 call sites). This touches `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`.
3. Do not snap a dial-lowered depth to more passes when the total is at most one depth (`aggressiveness.rs:228-240`).

**Confidence.** High. All values follow from the shipped JSON and the code above.

---

## Summary of root causes

| Item | File:line | Cause |
|---|---|---|
| a | `compute/catalog/registry.rs:824`, `feeds/mod.rs:3433`, `feeds/mod.rs:2110`, `feeds/suggest/apply.rs:160-183`, `feeds/suggest/aggressiveness.rs:220-250` | No 3D Rough depth hint. The rigidity floor 2 x D, then 0.8 x flute = 16, overwrites the operator's depth under `Both`. The dial gives x0.878 = 14.047. `total_depth: none` (`operation_configs.rs:3770`), so nothing sees the levels |
| b | `machine/mod.rs:628`, `feeds/mod.rs:2603-2672`, `feeds/operating_point.rs:205`, `tool_load/power.rs:257,312`, `feeds/profile.rs:17` | The default 6000 cap lowers the RPM, and with it the VFD power (1.01 kW). Pre-sim power is at nominal ae only. The sim peak needs a near-slot arc. No path returns it to Suggest (export refusal and the modulator power cap only) |
| c | `extrapolation/size.rs` (form C), `vendor_lookup.rs:537,573`, `feeds/mod.rs:2164-2209` | 0.1448 x 1.237 x 1.033. Chip thinning is measured, not applied (ruling), and A2 caps only at the same point. The row is a centre-cutting plunge series |
| d | `dressup/entry_descent.rs:434-479, 668-673`, `compute/tool_config.rs:374`, `rs_cam_cli/src/rough_score.rs:455` | Short air descents (<= 0.5 mm) or plunges <= the protrusion are allowed by design. rough-score labels a run by its first move. The open gap is the un-planned cut and link descents |
| e | `registry.rs:86`, `feeds/mod.rs:2994`, `aggressiveness.rs:232` | The pocket default ae 0.35 D replaces the facing stepover. The relative dial plus the depth snap halves a 0.5 mm face |
