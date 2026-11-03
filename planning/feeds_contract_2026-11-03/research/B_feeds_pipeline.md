# B. The feeds / Suggest pipeline as a data flow

Read-only research at HEAD `40cf411a7` (2026-11-03). No source edit, no cargo run.
Paths are under `crates/rs_cam_core/src/` unless they say otherwise. The feeds files
are not in the working-tree diff, so the working-tree line numbers equal HEAD.

Background read: `planning/feeds_blind_review_2026-11-03/rootcause/INDEX.md`, 01, 02, 05, 06.
This file does not repeat their per-op arithmetic. It maps the types and the default fills
that make those findings possible.

Sections 1 to 6 are facts with file:line. Section 7 holds the suggestions.

---

## 1. The flow, stage by stage

### 1.0 Overview

```
ProjectSession::cutter_op_profile (session/mod.rs:2323)
  -> suggest_slots (session/mod.rs:2294) -> SuggestSlots::context(&dressups)
  -> CutterOpProfile::for_combo (feeds/profile.rs:143)
       -> feeds_explain_for_operation (feeds/suggest.rs:1057)       [display payload]
       -> suggest_for_operation (feeds/suggest.rs:941)
            -> feeds_result_for_operation (feeds/suggest.rs:1036)
                 -> feeds_input_for_operation (feeds/suggest.rs:987)  [OperationConfig -> FeedsInput]
                 -> validate_tool_for_operation (feeds/mod.rs:1131)   [runtime refusal]
                 -> calculate (feeds/mod.rs:1647)                     [FeedsInput -> FeedsResult]
            -> apply(ApplicableRecommendation::validated, ApplyScope::Both) (feeds/suggest/apply.rs:881)
                 -> apply_feeds_subset (apply.rs:111)
                      -> enforce_invariants / _at_fixed_geometry (suggest/invariants.rs:140, 165)
                           -> run_invariants passes 0..10 (invariants.rs:189-341)
                      -> resolve_plunge, resolve_ramp_feed, provenance stamps
                 -> apply_drill_defaults (apply.rs:1069) under a geometry scope
       -> axial_envelope_for_operation (suggest/axial_envelope.rs:138)

Command::ApplyFeeds { index, scope, explored } (session/command.rs:1180, 3064)
  -> apply_to_operation (apply.rs:941) -> feeds_preview_for_operation (apply.rs:1012)
  -> FeedsPreview::try_applicable (apply.rs:764) -> apply (apply.rs:881)
  -> Command::ReplaceToolpathConfig
```

The profile path and the ApplyFeeds path run the same calculator and the same `apply`.
The profile writes into a clone and returns it (`CutterOpProfile::suggested_operation`,
`profile.rs:120`). `ApplyFeeds` writes into a draft and installs it.

### 1.1 Stage A: the project context (`session/mod.rs:2294-2347`)

| Item | Type | Value |
|---|---|---|
| Input | `&ToolpathConfig` | the op, its tool id, its dressups |
| Output | `SuggestContext<'a>` (`feeds/suggest.rs:150`) | built by `SuggestSlots::context` |
| `stock` | `Option<&StockContext>` | `Some`, from the stock bbox and padding (`suggest.rs:52-62`) |
| `StockContext::stock_z` | `Option<f64>` | `None` when the stock bbox is flat (S11, `suggest.rs:53-55`) |
| `model_bbox` | `Option<&BoundingBox3>` | the bbox of `tc.model_id`; read only by the pass 3 run-time back-off (`invariants.rs:417`) and the strategy class (`adaptive_entry.rs:77`) |
| `dressups` | `Option<&DressupConfig>` | `Some(&tc.dressups)`; read only for the ramp entry angle |
| `upstream_leftover_stock_mm` | `Option<f64>` | always `None`; no production reader (`rg` finds only comments and one test) |
| `neighboring_strategy_hint` | `Option<&str>` | always `None`; no production reader |
| `chipload_bounds`, `matched_lut_row`, `effective_diameter_mm`, `calculator_operating_point` | filled later by `apply_feeds_subset` (`apply.rs:229-240`) | `effective_diameter_mm: f64` uses `0.0` as "not set" (sentinel contract, `suggest.rs:183-200`) |

Facts:

- The heights (`top_z`, `bottom_z`, the floor) and the level structure never reach Suggest.
  The only Z data is the stock bbox.
- `spindle_strategy` comes from `post_config().spindle_strategy` on the profile and
  `ApplyFeeds` paths (`session/mod.rs:2342`, `command.rs:3080`). The draft door
  `session/op_draft.rs:652` and the tier planner `session/multitool.rs:891` pass
  `SpindleStrategy::default()` (`MatchChart`), and the draft door passes no stock slot
  (`op_draft.rs:653-656`), so `apply_drill_defaults` gets `stock: None` there.

### 1.2 Stage B: `OperationConfig` -> `FeedsInput` (`feeds/suggest.rs:987-1025`)

| `FeedsInput` field (`feeds/mod.rs:378-428`) | Source | Option / None meaning |
|---|---|---|
| `tool_diameter`, `flute_count`, `flute_length`, `shank_diameter` | `ToolConfig` | `shank_diameter` is always `Some` here |
| `tool_geometry` | `tool.geometry_hint()` | |
| `tool_series` | `ToolSeries::for_cut_direction(tool.cut_direction)` | `None` = generic upcut |
| `operation` (`OperationFamily`), `pass_role` | `operation.feeds_style()` (registry row) | |
| `operation_kind` | `Some(op_type)` | `None` only in unit fixtures |
| `axial_depth_mm` | `operation_feeds_hints(op).0` (`apply.rs:1196`) | `None` = "auto-calculate" |
| `radial_width_mm` | `operation_feeds_hints(op).1` | `None` = "auto-calculate" |
| `target_scallop_mm` | `operation_feeds_hints(op).2` | `None` = no scallop rule |
| `vendor_lut` | `Some(lut)` | |
| `setup.tool_overhang_mm` | `Some(tool.stickout)` | |
| `spindle_strategy` | caller | |

The hints come from `Capabilities::feeds_hints` (`compute/catalog.rs:1916-1936`). The
registry gives `FeedsHints::NONE` to every op row (`compute/catalog/registry.rs:86, 135, 186,
240, 303, 362, 421, 470, 522, 577, 653, 714, 769, 824, 873, 927, 979, 1046, 1099, 1150, 1206,
1255, 1311, 1364, 1424`). `OperationConfig::capabilities` overrides only seven ops
(`compute/catalog.rs:1006-1068`):

| Op | Hint |
|---|---|
| Scallop, UnifiedFinish | `target_scallop_mm = scallop_height` |
| DropCutter | `target_scallop_mm = scallop_height` (an `Option`) |
| Waterline, SteepShallow | `axial_depth_mm = z_step` |
| VCarve | `axial_depth_mm = max_depth` (an `Option`) |
| RampFinish | `axial_depth_mm = max_stepdown` |

Facts:

- No op sends `radial_width_mm` (`compute/catalog.rs:1920`: "none of the current ops set this").
- No op sends its own `depth_per_pass` or `stepover`. The operator's cut geometry does not
  enter the calculator. It enters later only under `ApplyScope::Speeds` (section 1.5).
- The lib test `operation_feeds_hints_is_an_explicit_per_op_decision`
  (`feeds/suggest/tests.rs:2544`) pins this table.

### 1.3 Stage C: the refusal gate (`feeds/mod.rs:1131-1160`, `feeds/support.rs`)

- Input `&FeedsInput`, output `Result<(), FeedsError>`.
- `feeds_support(input)` (`support.rs:533`) joins the static formula source of the op with
  the recipe row lookup (`recipe_row_lookup`, `support.rs:464`). A `FeedsSupport::Refuse`
  arm gives `FeedsError::Unbacked` (`mod.rs:1155-1157`).
- The gate decides only "is there a chipload basis for this cell". It does not check
  the geometry inputs. A cell with no depth, no width and no hole depth passes.
- `calculate` itself runs on any input. `FeedsPreview::build` (`apply.rs:718`) and
  `feeds_explain_for_operation` run it on a refused cell to show a "would-have-produced"
  preview (`profile.rs:111-116`).

### 1.4 Stage D: the calculator `calculate` (`feeds/mod.rs:1647-2970`)

Input `&FeedsInput`. Output `FeedsResult` (`mod.rs:465-577`): plain `f64` for `rpm`,
`chip_load_mm`, `feed_rate_mm_min`, `plunge_rate_mm_min`, `axial_depth_mm`,
`radial_width_mm`, `power_kw`, `available_power_kw`, `mrr_mm3_min`,
`effective_diameter_mm`; `Option` for `vendor_source`, `chipload_bounds`,
`chipload_point_mm`, `matched_lut_row`; enums `support`, `chipload_source`, `plunge`,
`ramp`; `warnings: Vec<FeedsWarning>`; `derates: FeedsDerates`.

No output number carries its source in its type. Each number is an `f64`.

| Step | Lines | What it computes | Default fill when data is missing |
|---|---|---|---|
| pre | 1691-1696 | engaged diameter for the SFM | `axial_depth_mm.unwrap_or(d)`: no depth hint means "1 x D deep" (1691) |
| 1 RPM | 1697-1729 | `ideal_rpm = Vc x 1000 / (pi x De)` | `Vc` = `material.base_cutting_speed_m_min()` (`material/mod.rs:1334`; solid wood 200 m/min, marked unsourced); `FALLBACK_RPM` when `De <= 0` (1315); drill tiers `drill_rpm_envelope_for_diameter` (repo-authored, 1381) |
| 2 chip | 1731-1946 | chip from the vendor row, else the formula | formula `k0 x De^p x (1/H)^q` from `machine.chip_load` (1745; `k0` uncited, `support.rs:36-39`); band rows ship the **midpoint** `chipload_midpoint x scale` (`vendor_lookup.rs:658-662`); a refused G1 row gives `chip_load_mm = 0.0` as a sentinel (`vendor_lookup.rs:658-660`) and the formula takes over (1853); an RPM-only row keeps its RPM and takes the formula chip (1870ff) |
| 2 RPM | 1945 | `rpm = clamp(row rpm_nominal)` under `MatchChart` | the row RPM replaces the SFM RPM whenever the row prints one |
| 2b | 1948-2050 | `MaxSpeed` walk up the constant-chip line | caps `MAX_SPINDLE_SPEEDUP` 1.5, `SPINDLE_CEILING_HEADROOM` 0.95 (`mod.rs:366, 372`) |
| 2c | 2052 | drill RPM clamp again | |
| 3 ap/ae | 2075-2077, `operation_default_profile` 2978-3083, `default_engagement` 3360-3494 | `ap = d x ap_factor`, `ae = d x ae_factor` | **family default table** per `(OperationFamily, PassRole)` (2978-3083); Adaptive: `ADAPTIVE_*` bases (3224ff, unsourced), hardness scales; **rigidity floor** `ap_factor = max(.., adaptive_doc_factor x profile.ap_factor / 1.5)` (3433-3436); wood flat/bull `ae = max(.., adaptive_woc_factor)` then `min(adaptive_woc_factor)` (3449-3451); finish ball/tapered overrides `FINISH_*` (3456-3488); floors `ap >= 0.05`, `ae >= 0.02` (3491-3492) |
| 3b scallop | 2079-2100 | `ae = scallop_stepover(r, h)` | only with a scallop hint; else the family `ae` stays |
| 3 hints | 2102-2108 | a hint replaces `ap` / `ae` | only for the seven ops above |
| 4 flute guard | 2110-2121 | `ap <= 0.8 x flute_length` | `FLUTE_GUARD_FACTOR` 0.8 (1539, unsourced); no flute length: `FLUTE_GUARD_NO_LENGTH_D` x D (1545) |
| 4 floors | 2124-2128 | `ap >= MIN_AP_MM`, `ae >= MIN_AE_MM`, `ae <= d` | |
| 4b slot | 2130-2139 | `ae > SLOTTING_THRESHOLD x d` caps `ap` | 1551, 1557 (unsourced); never fires when `ae` is a family default under the threshold |
| 4c shank | 2141-2149 | warning only | |
| 5 feed | 2151-2233 | `raw_feed = rpm x chip x Z x depth_tier` | chip thinning computed, **not applied** (2205-2212); `depth_tier = depth_tier_multiplier(ap, ladder_d)` (2226-2232) reads the **default** `ap` when no hint exists |
| 5b L/D | 2235-2254 | `long_tool_load_share` (1633): 0.88 above 4 x D, 0.75 above 6 x D | plain `const f64` (1619-1625), marked "repo rule, unsourced", outside `FormulaConstant`; warning only, the feed does not take it |
| 6 power | 2256-2585 | power at `(ap, ae, rpm, feed)`; ladder: RPM, then `ap`, then `ae`, then nothing | rungs 2 and 3 move `ap` / `ae` only when the hint is `None` ("where the ENGINE chose it", 2480, 2490); power at the **default** engagement |
| 7 feed cap | 2589-2683 | ceiling `cutting_feed_ceiling_mm_min()`; RPM follows the feed down (R4 Q10) | `DEFAULT_CUTTING_FEED_CAP_MM_MIN` 6000 when the machine sets none (`machine/mod.rs:628`); `fits_power` reads the default `ap` / `ae` (2611-2620); RPM floor = row `rpm_min`, else machine minimum (2606-2609) |
| 8 plunge | 2685-2695 | `plunge_basis(input, material.plunge_rate_base(d))` | G10 claim fraction of F; else **material base** (`material/mod.rs:1369`, named repo rule, diameter scale clamp) |
| 8b ramp | 2697-2701 | `ramp_basis(input)` | `RampBasis::NoChip` when no G6 claim |
| 9 | 2703-2714 | no factor (R4); `plunge = resolve_plunge(basis, feed)` | |
| 9b | 2716ff | rubbing floor `min(0.025, band min)` warns | `RUBBING_FLOOR_MM_TOOTH` 0.025 (1184, repo constant) |
| 9c | 2817ff | drill envelope; feed is plunge | |

### 1.5 Stage E: `apply_feeds_subset` (`feeds/suggest/apply.rs:111-401`)

Input: `&FeedsResult` (wrapped in `ApplicableRecommendation`), `&mut OperationConfig`,
`&mut FeedsProvenance`, `ApplyScope`, `SuggestContext`. Output: `Vec<SuggestWarning>`.

1. Scratch clone. Feed and plunge round **down** to 1 mm/min (T-9, 137-142).
2. Stepover and depth round to 0.001 (156-157) and go onto the scratch.
3. Depth snap: `realised_step_down(total, depth)` when `scratch.total_depth()` is `Some`;
   else the raw number stays (178-181). 3D Rough has `total_depth: none`
   (`compute/operation_configs.rs:3770`, per report 01).
4. RPM written when finite (196-199).
5. Context enriched with the band, row, effective diameter and the calculator point
   (229-240). `calculator_operating_point` is withheld for explored speeds (233).
6. Scope split (247-268):
   - `Speeds`: the scratch gets the operation's own stepover and depth back, and
     `enforce_invariants_at_fixed_geometry` runs (geometry passes off; pass 9 rescales
     the feed to the kept geometry; G-SPEEDSAPPLY).
   - `Both`, `CutGeometry`: `enforce_invariants` runs the full chain on the
     calculator's geometry.
7. Plunge re-derived at the final feed (277-293), rpm rationale row (297-319).
8. Copy back: speeds under `writes_speeds()` (324-330); ramp feed (340-361); stepover and
   depth under `writes_geometry()` (362-388).
9. Provenance stamps (393-401).

Facts:

- Under `Both` the calculator's family-default `ap` / `ae` overwrite the operator's
  `depth_per_pass` and `stepover`. Nothing compares them with the operator's values first.
- `CutGeometryFieldNotHeld` warns when an op has no field for the value (375-387). Profile
  and Project Curve have no stepover, so their calculator `ae` is used for power and then
  dropped.

### 1.6 Stage F: `run_invariants` (`suggest/invariants.rs:189-341`)

Order. "Geo" means the pass does not run under `Speeds`.

| Pass | Function (file:line) | Reads | Writes | Default fill |
|---|---|---|---|---|
| 0 geo | `pick_axial_envelope` (`axial_envelope.rs:261`) | op depth, matched row `ap_max_factor` | depth (down only) | envelope width: Roughing3d `stepover.unwrap_or(0.4 x D)` (`axial_envelope.rs:166`); VBitGroove `0.2 x D` without a depth hint (`:198, 201`); CurveRough `0.2 x D` (`:214`); fz `0.0` when the op has no feed (`:157-158`) |
| 0 re-band | `recompute_chipload_bounds_for_dpp` (`axial_envelope.rs:364`) | | `chipload_bounds` | |
| 1 | `clamp_plunge_to_feed` (`invariants.rs:344`) | | plunge <= feed | |
| 2 geo | `clamp_stepover_to_diameter` (362) | | stepover <= D | |
| 3 geo | `backoff_stepover_for_runtime` (404) | `model_bbox`, `predict_move_count` | stepover up | target 500 000 moves, 0.5 x D cap (84-102) |
| 4 geo | `clamp_dpp_to_rigidity` (475) | `RigidityProfile::depth_cap_mm` | depth down | roughing only; adaptive family `adaptive_doc_factor`, else `doc_roughing_factor` |
| 5 geo | `clamp_dpp_to_cutting_length` (561) | `tool.cutting_length` | depth down | |
| 6 geo | `backoff_dpp_for_deflection` (599) | deflection predictor | depth down | target 200 um, floor 0.5 mm (52, 60) |
| 6b geo | `apply_aggressiveness` (`aggressiveness.rs:146`) | `machine.aggressiveness` x `long_tool_load_share` | depth and stepover by one scale | base `ap0 = depth.unwrap_or(calc.ap)`, `ae0 = stepover.unwrap_or(calc.ae)` (`aggressiveness.rs:200-206`); rpm `unwrap_or(calc.rpm)` (184-188); no action on Finish or Drill (169-180); a scallop-set stepover is not a lever (195-199) |
| warn | `pick_adaptive3d_clearing_strategy` (`adaptive_entry.rs:55`), `check_plunge_entry_stability` (110) | | nothing | |
| 9 | `rescale_feed_to_final_geometry` (`adaptive_entry.rs:228`) | calculator point vs final geometry | feed (depth tier only) | skipped when `calculator_operating_point` is `None` |
| 10 | `recheck_power_after_rescale` (`adaptive_entry.rs:456`) | `power_at_operating_point` | feed down | only when pass 9 moved the feed |

### 1.7 Stage G: after the chain

- `apply_drill_defaults` (`apply.rs:1069-1121`) under a geometry scope: `peck_depth =
  material.drill_default_peck_depth_mm(d)` (half the Janka-band per-peck maximum,
  `material/mod.rs:1444, 1481`), capped by `peck_for_hole` (`apply.rs:1123`) at
  `0.75 x hole depth`. Drill reads `cfg.floor.depth_below_top()`; the pin drill reads
  `stock.stock_z + spoilboard_penetration`. With no depth the peck is not capped.
- `apply_stock_defaults` (`apply.rs:1138`) runs only in `default_operation` (add door,
  `suggest.rs:928`): DropCutter `min_z = stock_bottom_z`; Face `depth =
  stock_padding.max(1.0)`; Profile and Drill `floor = top(stock_z)` when `stock_z` is `Some`.

### 1.8 Every default fill, in one list

Each line is a place where a missing input becomes a number with no operator value behind it.

1. `axial_depth_mm.unwrap_or(d)` for the SFM diameter (`mod.rs:1691`).
2. Material cutting speed when the row prints no RPM (`mod.rs:1698-1702`; `material/mod.rs:1334`).
3. `FALLBACK_RPM` when the engaged diameter is not positive (`mod.rs:1315, 1701`).
4. Drill RPM tiers (`mod.rs:1381`, repo-authored).
5. Formula chipload `k0 x De^p x (1/H)^q` when no row, an RPM-only row, or a refused row (`mod.rs:1745`).
6. The band midpoint as the shipped chip (`vendor_lookup.rs:661`); midpoint vs maximum left open by ruling (`mod.rs:2196-2200`).
7. The row RPM replaces the SFM RPM under `MatchChart` (`mod.rs:1945`).
8. Family default `ap` / `ae` table (`mod.rs:2978-3083`).
9. Adaptive bases and hardness scales `ADAPTIVE_*` (`mod.rs:3224-3330`, unsourced).
10. Rigidity **floor** on adaptive `ap` (`mod.rs:3433-3436`).
11. Wood flat/bull adaptive `ae` floor and cap (`mod.rs:3443-3451`).
12. Finish ball / tapered overrides (`mod.rs:3456-3488`).
13. `ap >= 0.05`, `ae >= 0.02`, `ae <= D` (`mod.rs:2125-2128, 3491-3492`).
14. Flute guard 0.8 x flute length, or `FLUTE_GUARD_NO_LENGTH_D` x D (`mod.rs:2110-2121`).
15. Slot threshold and slot depth cap (`mod.rs:2130-2139`).
16. Depth tier at the default `ap` (`mod.rs:2226-2232`).
17. L/D share 0.88 / 0.75 (`mod.rs:1619-1645`) into the dial (`aggressiveness.rs:160-161`).
18. Default cutting-feed cap 6000 mm/min (`machine/mod.rs:628`).
19. RPM floor = row `rpm_min`, else machine minimum (`mod.rs:2606-2609`).
20. Plunge material base (`material/mod.rs:1369`) when no G10 claim covers the tool.
21. G10 plunge = fraction x shipped feed (`plunge.rs:308`), F / Z for flat (`extrapolation/plunge.rs:631`), re-run at the final feed (`apply.rs:277-293`).
22. Ramp feed = min(F, plunge / tan theta) with no G6 chip (`ramp.rs:627`).
23. Rubbing floor 0.025 (warn only, `mod.rs:1184`).
24. Pass 0 envelope widths 0.4 x D / 0.2 x D and fz 0.0 (`axial_envelope.rs:157-214`).
25. Pass 3 move budget 500 000 and 0.5 x D cap (`invariants.rs:84-102`).
26. Pass 4 rigidity cap factors (`machine/mod.rs:143-163` per report 05).
27. Pass 6 deflection target 200 um, floor 0.5 mm (`invariants.rs:52, 60`).
28. Pass 6b dial 0.85 (`MachineProfile::aggressiveness`) and the fallbacks to the calculator point (`aggressiveness.rs:184-206`).
29. Peck = half the per-peck maximum, capped 0.75 x hole (`apply.rs:1069-1135`).
30. Face depth = `max(stock_padding, 1.0)` on an add (`apply.rs:1143-1145`).
31. Calculator point as the power fallback in the GUI card (`rs_cam_viz/src/ui/feeds/compare.rs:860-867`), none in the CLI (`rs_cam_cli/src/suggest.rs:51-56`).

`formula_constants()` (`mod.rs:3098`) lists most of these numbers as `FormulaConstant`
(`material/mod.rs:677-725`). The sentry pins 144 `Unsourced` rows
(`tests/every_feeds_formula_constant_states_its_source_f201.rs:39`). Items 17, 25, 27 and the
pass 0 widths are plain constants outside that list.

---

## 2. What Suggest writes, under which scope, from which surface

### 2.1 Fields

| Field | Writer | `Speeds` | `CutGeometry` | `Both` |
|---|---|---|---|---|
| `feed_rate` | `apply.rs:325` | yes | no | yes |
| `plunge_rate` | `apply.rs:326` | yes | no | yes |
| `spindle_rpm` | `apply.rs:327-329` | yes (when finite) | no | yes |
| `ramp_feed_rate` | `apply.rs:349` | yes (op with the field) | no | yes |
| `stepover` | `apply.rs:363-365` | no | yes | yes |
| `depth_per_pass` | `apply.rs:366-368` | no | yes | yes |
| `peck_depth` (Drill, AlignmentPinDrill) | `apply.rs:896-898` | no | yes | yes |
| `FeedsProvenance` stamps | `apply.rs:393-401` | speed fields | geometry fields | all |

Not written: the entry style (Q11, `suggest.rs:117-121`), `scallop_height`
(`provenance.rs:167-168`), the clearing strategy (warn only), stock to leave, the floor and
the heights. `SuggestScope::StrategyAndFeeds` (`suggest.rs:109-122`) is the default but no
strategy write remains.

`ApplyScope` (`apply.rs:46-68`) has no per-field arm (Checkpoint I-1, `apply.rs:28-36`).

### 2.2 Surfaces

| Surface | Call site | Path | Scope |
|---|---|---|---|
| GUI Apply (modal, batch) | `rs_cam_viz/src/controller/events/mod.rs:1074` | `Command::ApplyFeeds` | `Both` (1134, 1257), `Speeds` (1161), `Speeds` + explored (1311) |
| GUI card | `rs_cam_viz/src/ui/feeds/compare.rs:110, 146, 866` | preview, `suggest_for_operation`, power with calc fallback | display |
| GUI preview | `rs_cam_viz/src/state/runtime.rs:392` | `feeds_preview_for_operation` | display |
| GUI pin auto-create | `rs_cam_viz/src/controller/events/pins.rs:570` | `suggest_params` | `Both` |
| MCP `apply_feeds` | `rs_cam_viz/src/app/mcp/generation.rs:873`; scope map `rs_cam_mcp/src/server.rs:412-414` | `Command::ApplyFeeds` | caller's |
| MCP `get_suggest_rationale` | `generation.rs:215` | `cutter_op_profile` | display |
| CLI `--apply-suggest` | `rs_cam_cli/src/project.rs:1333` | `Command::ApplyFeeds` | `Both` |
| CLI `suggest` | `rs_cam_cli/src/suggest.rs:117`, power `:51` (fallback `None`) | `cutter_op_profile` | display |
| CLI smoke | `rs_cam_cli/src/smoke.rs:501` | `suggest_params` | `Both` |
| Core add door | `session/op_draft.rs:645` | `suggest_params` | `Both` |
| Core tool bind (R1) | `session/mutation/toolpath.rs:905-975` `seed_feeds` | `apply_to_operation` | `Speeds`, only EMPTY or SUGGESTED fields |
| Core export records | `session/compute/export.rs:752` | `suggest_for_operation` | records only |
| Core strategy advisor | `session/compute.rs:1683` | `suggest_for_operation` | `Both` on a clone |
| Core tier planner | `session/multitool.rs:891` | `suggest_for_operation` | `Both`, then re-applies its own geometry |

---

## 3. Provenance and support types

| Type (file:line) | What it encodes | Compiler-checked? |
|---|---|---|
| `FeedsSupport` (`support.rs:369-401`) | the chipload basis of the cell: `VendorBacked`, `Extrapolated{Claim}`, `FamilyTransferred`, `DrillTransferred`, `FormulaOnly{source}`, `Refuse{reason}` | No. A runtime gate (`mod.rs:1155`) turns `Refuse` into `FeedsError::Unbacked`. `calculate` still returns numbers for a `Refuse` cell. The arm says nothing about `ap`, `ae`, RPM, plunge or power. Displayed by `card_text` (`support.rs:407`). |
| `ChiploadSource` (`mod.rs:431-435`) | `VendorLut{id}`, `FormulaFallback`, `EdgeRadiusFloor` | No. `EdgeRadiusFloor` has no producer (only matched at `mod.rs:2904`, `provenance.rs:132`). |
| `SizeBasis` (`extrapolation.rs:313-335`) on `LookupResult::size_basis` | G1: `Exact`, `NoDiameterAnchor`, `NoChipload`, `AngleKey`, `Claim(Box<Claim>)`, `Refused` | No. The band numbers sit beside it as plain `f64` / `Option<f64>` (`vendor_lookup.rs:50-118`); a refused basis is enforced by a `0.0` sentinel and `None` (`vendor_lookup.rs:655-672`), and each consumer checks `is_refused()`. |
| `Claim` (`extrapolation.rs:197`), `FamilyClaim` (`extrapolation/family.rs:167`), `DrillClaim` (`drill.rs:150`), `HardnessBasis` (`hardness.rs:105`), `PlungeClaim` (`extrapolation/plunge.rs:396`) | G1/G2/G3/G6/G10 claim records: rule, range, scale, anchor, residual | No. Card and FM1 text. |
| `PlungeBasis` (`plunge.rs:81-96`) + `PlungeBinding` (100) | `Claimed{claim, tip_cap}`, `MaterialBase{..}`, `DrillCycle`; the binding term | Partly. The plunge VALUE is computed only through `resolve_plunge(basis, feed)` (`plunge.rs:306`), so a plunge cannot exist without a basis. The basis does not record WHICH row the feed came from, so a claim from one row multiplies a feed from another (report 02 (c)). |
| `RampBasis` (`ramp.rs:61-75`), `RampFeed` (480), `RampArm` (413) | `Sourced{axial chip}` or `NoChip{reason}`; which term set the ramp | Partly, as `PlungeBasis`: the value comes from `resolve_ramp_feed` (`ramp.rs:627`). |
| `FeedsProvenance` + `ValueProvenance` + `ProvenanceSource` (`provenance.rs:31-160`) | per-field stamp persisted with the op: `VendorLut`, `Formula`, `EdgeRadiusFloor`, `Manual`, `AutoCorrect`, `PublishedRule`, `RepoRule` | No. Labels are derived after the fact by `FeedsResult::provenance` (`provenance.rs:352-398`). `depth_per_pass` is labelled `VendorLut` whenever the row prints any `ap_*` field, and `Formula` otherwise (`:369-374`), even when the value came from the family table, the rigidity floor, the flute guard or the dial. No label says "family default" or "operator hint". Read by `seed_feeds` (`is_suggested`) and by the export records (`export.rs:715-731`). |
| `SuggestWarning` (`suggest.rs:250-770`) | the record of each pass that moved a value | No. Display. |
| `SuggestRationale` (`rationale.rs:216`, `from_warnings` 226) | the card tree | Yes for completeness: an exhaustive match over `SuggestWarning` (`rationale.rs:3-8`). It does not check that a value has a record. |
| `FeedsWarning` (`mod.rs:777`), `FeedsDerates` (`mod.rs:580`) | calculator records; `combined_factor()` is the only correct composition (`mod.rs:570-577`) | No. |
| `FormulaConstant` (`material/mod.rs:677-725`) | `Cited` or `Unsourced` per constant | No. A count test pins 144 unsourced rows. |
| `BoundSource` (`tool_load/verdict.rs:1019-1050`) | post-sim bound source: `MachinePowerCurve`, `VendorChipBand`, `DeflectionBudget`, `RigidityRuleOfThumb` | Runtime: `gates_export()` (`verdict.rs:1142`) lets every source but the rule of thumb refuse an export. |
| `EvidenceGrade`, `ObservationKind` (`vendor_lut.rs:70-90`) | row grade A/B/C and exact/derived/fallback | Runtime: `validate_observation` (`vendor_lut.rs:579`) at LUT load. |

Types that the compiler does check:

- `ApplicableRecommendation` (`apply.rs:781-805`): private field, constructors
  `validated` (`pub(super)`) and `FeedsPreview::applicable`. Holding one proves that
  `validate_tool_for_operation` passed. This is the one type-level proof in the pipeline.
- `ApplyScope` as an enum (F025, `provenance.rs:186-188`): a swap of two bools cannot compile.
- `feeds/quantities.rs:71-229`: `CommandedFeedMmMin`, `AchievedFeedMmMin`,
  `AdvancePerToothMm`, `ArcMeanChipThicknessMm`, `VendorChiploadBand`. They keep the
  arc-mean chip apart from the advance per tooth on the gate -> UI boundary. Suggest
  internals do not use them.
- The `test-support` gate on the raw-result apply doors (F042, `apply.rs:413-480`).

Summary: the engine records the source of the CHIP well (claims, support arm, grade).
It records the source of `ap`, `ae`, RPM and power only as free text in warnings, or
not at all. No type stops a family default from flowing into a shipped number.

---

## 4. Power and load models

### 4.1 One formula, one force line

- Force line: `Material::force_line()` (`material/force_line.rs`; ruling B6). Power =
  line x grain factor (1.0).
- Power: `tool_load::power::PowerTerms::of(PowerModelInputs)` (`tool_load/power.rs:83-235`),
  two terms: shear `Ks x MRR` and edge `F_edge x ap x Vc x z psi / 2 pi`.
- Immersion: `feeds::force::immersion_angle(ae, r)` (`feeds/force.rs:57`).
- Lateral force: `feeds::force::lateral_cutting_force` (`force.rs:75-93`), with
  `theta_peak = min(psi, pi/2)`.
- Load fraction `L`: `tool_load::load_fraction::chip_load_fraction` (`load_fraction.rs:36`).

### 4.2 Several engagement inputs

The formula is shared. The inputs that feed it are not.

| Consumer | `ap` from | `ae` / psi from | Engagement diameter | Cross-section |
|---|---|---|---|---|
| Calculator Step 6 / Step 7 `power_model_terms` (`mod.rs:1515-1536`, 2284, 2611-2620) | hint, else family default, after the flute guard | hint, else family default; `psi = immersion_angle(ae, De/2)` | `effective_diameter` (chip-thinning circle, `mod.rs:2160-2165`) | `tool_geometry.mrr_cross_section_mm2(ap, ae)` |
| `power_at_operating_point` (`operating_point.rs:205-365`): CLI, GUI card, pass 6b, pass 10 | `op.depth_per_pass()`, else fallback | `op.stepover()`, else fallback; else `Err(NoRadialEngagement)` (`:294-301`) | `effective_diameter` at the final `ap` (`:337`) | geometry hint `mrr_cross_section_mm2` |
| Pass 6b force (`aggressiveness.rs:76-101`) | trial `ap` | trial `ae` | `effective_diameter` | `lateral_cutting_force` |
| Post-sim gate `sample_power_kw` (`tool_load/power.rs:257-300`), `evaluate` (`:312`) | `sample.axial_doc_mm` (dexel) | `sample.arc_engagement_radians` (dexel); slab width `= (psi / pi) x 2 r_eng` (`:272-273`) | `tool.engagement_radius(ap) x 2` | `tool.mrr_cross_section_mm2(ap, slab width)` |
| Feed modulator power cap (`dressup/feed_modulation.rs:563-617`) | `effective_axial_mm(engagement, ctx)` (`:720`) | `radial_woc_fraction` per move (`:482`); `radial = woc x D_eng`; `psi = immersion_angle(radial, D/2)` | `PowerLimitInputs::engagement_diameter_mm` (`session/compute.rs:2172-2177`) | `axial x radial` (a rectangle; no V-bit shape) |
| Deflection predictor (`feeds/predict.rs:464-472`), cutter constraints (`cutter_constraints.rs:288, 580, 697`) | caller | `immersion_angle(ae, diameter/2)` / `tool.radius()` | nominal radius | |

Facts:

- The gate's slab width is the arc-length equivalent `(psi/pi) x D`. The pre-sim and
  modulator widths are the geometric `ae` (`psi = acos(1 - 2 ae/D)`). At psi = pi/2 they
  agree (D/2). At psi = pi/3 the gate uses D/3 and the geometric width is D/4. So the shear
  term of the gate and of Suggest differ for the same cut.
- The modulator cross-section is `axial x radial` for every tool; the gate and Suggest use
  the cutter's shape-correct section.
- Pre-sim power runs only at ONE point: the nominal engagement. No adaptive excursion,
  no slot, no entry. Report 01 (b) measured 0.53 kW pre-sim vs 1.18 kW in the sim.
- Profile and Project Curve have no stepover. Pre-sim power is then the calculator's
  family-default `ae` (GUI) or a refusal (CLI).

### 4.3 Where the post-sim result goes

- `tool_load::evaluate_toolpath` -> `ToolpathLoadVerdict` -> export refusal
  `enforce_exceedance_policy` (`gcode/mod.rs:827`) unless the operator accepts
  (`gcode/mod.rs:54`, `accept_exceeded_tool_load`). Only a sourced bound refuses
  (`verdict.rs:1142`; sentry `a_weak_bound_cannot_refuse_an_export_g_s4weak`).
- The modulator caps each move at power, deflection, chip band, machine feed and reach
  (`feed_modulation.rs:563-640`). It writes the export IR, not `feed_rate`.
- No path returns a verdict to Suggest. `profile.rs:15-18` states that the profile "must not
  own the post-sim `ToolpathLoadVerdict`". `feeds/CLAUDE.md` states: "On an adaptive rough
  the simulated chipload outranks the Suggest verdict."

---

## 5. Rulings in force

| Ruling | Source | What it forbids or requires |
|---|---|---|
| No number from memory | root `CLAUDE.md` "Working preferences"; memory `feedback_no_bench_rig.md` | A constant is derived by math or printed in a source in the repo. No bench rig. |
| F201 | `mod.rs:3098`; `tests/every_feeds_formula_constant_states_its_source_f201.rs:39` | Every formula constant is `Cited` or `Unsourced`; the unsourced count (144) is pinned. |
| One load definition | memory `feedback_reuse_existing_load_calcs.md` (operator, 2026-10-27) | Build every load or engagement measure from the existing functions; one definition codebase-wide. |
| R1 (2026-09-23) | `planning/feeds_matrix_2026-09-23/RULINGS.md:12-21, 45-47`; `support.rs:14-20` | A formula-only cell ships only where an agent judgement found a figure within 0.5x-2x; else it refuses. The code reads the table; it does not judge. |
| R2 | `RULINGS.md:22-25, 52-55` | No depth ceiling on finishing passes; depth reported; deflection decides. |
| R3 | `RULINGS.md:26-29` | The 1.5 x D static hint is deleted; one published depth ladder. |
| R4 (second round) | `RULINGS.md:38-44`; `mod.rs:2703-2714` | No hidden safety factor. The dial is the only margin. Keep the chip; cut load through engagement. |
| R4 Q1-Q11 | `RULINGS.md:56-81` | Q1 dial 0.85; Q2 ceiling = `power_at_rpm`, no fraction; Q3 above 1.0 warns; Q4 no dial on Finish; Q5 plunge no factor; Q7 L/D into the dial target, never the feed (`mod.rs:2235-2240`); Q8 workholding factor gone (`mod.rs:327`); Q9 rubbing floor warns, never lifts; **Q10 RPM follows the feed down to hold the chip, bounded by row and machine minimum** (`mod.rs:2589-2600`); Q11 one common scale on depth and width. |
| No invisible calculation | `RULINGS.md:79-81` | Every scale that moves a number shows on the card with its source status. |
| Micro extrapolation refuses | `RULINGS.md:94-101` | Under 1.5 mm with only a row more than 2x off: refuse. |
| Wood families do not substitute | `RULINGS.md:102-106` | Softwood, hardwood, plywood, MDF charts stay apart; Janka is the in-family dial. |
| G-CHIPTHIN-HALFFIX (2026-08-19) | `mod.rs:2163-2212`; `dressup/feed_modulation.rs:484-490` | Chip thinning is measured, not applied, in Suggest and in the modulator: no wood chart prints a radial condition. Midpoint vs band maximum left open (`mod.rs:2196-2200`). |
| Checkpoint J-1 (2026-08-13) | `invariants.rs:293-296` | Pass 8 feed-up recalibration retired; no pre-sim lift. |
| A1, A2, A3, A4 | `planning/extrapolation_2026-09-24/RULINGS.md:8-16` | A1 tapered row keyed at the tip; A2 a single printed value is a point, no derived band; A3 one row per cell, a G3 claim serves other families; A4 the "Wood" column serves hardwood as derived. |
| B1 G1 size | `extrapolation RULINGS.md:132-148` | Per-source interpolation and series slope; the generic 0.61 only inside 0.5-2x; else refuse; tip under 0.5 mm refuses. |
| B2 G2 hardness | `:150-167` | One Janka table; no hardness scale on plywood/MDF; cap the transfer at the printed soft/hard ratio. |
| B3 G3 family | `:169-185`; `feeds/CLAUDE.md` G3 | No copy rows, no ball rule; `FAMILY_RULES` only. ZrN v8 tapered rows serve contour, scallop, trace (2026-10-01). |
| B4 V-bit key | `:186-193` | A V-bit row is read at its printed key; V-bit parallel finish refuses. |
| B5 G6 drill | `:195-207`; `apply.rs:1080-1088` | Delete the 2.5 drill multiplier; flat end mill plunge chip = side chip / Z (Amana Ramp Down); refuse every other drill cell. Peck is a named repo rule. |
| B6 force line | `:209-221`; `material/CLAUDE.md` | One force line per material; no `Kc`, no anchor; refuse plywood, particleboard, plastics. Never type an envelope value as a literal. |
| B7 L/D | `:223-232` | Replace the 0.88/0.75 share with the deflection model after the envelope covers 2D ops. Open. |
| B8 G9 | `:17-22, 234-247` | Machine-vendor rows stay parked; the dial is the one margin. |
| Hardwood V-bit formula refuses (2026-09-25) | `:386-388` | Face, Pocket, Profile, Rest, Zigzag, ProjectCurve on a V-bit in hardwood refuse. |
| G10 Q1-Q12 (2026-09-25) | `:390-421` | Q2 ramp = min(F, plunge / tan theta); **Q4 flat plunge = side feed / Z**; Q5 ball/tapered 0.50, 60 deg V 0.33; Q6-Q10 named entry rules; **Q11 Suggest never writes the entry style**. |
| Project Curve routing (2026-09-25) | `feeds/CLAUDE.md` G3 | Ball, tapered, bull -> (Parallel, Finish); V-bit -> (Trace, Finish); facing bit refuses. |
| Series (2026-10-31) | `feeds/CLAUDE.md` G3; `vendor_lookup.rs:818` | The series is a tie-breaker only; the score decides every other choice. |
| S11 (2026-10-23) | `apply.rs:1158-1162`; `session/CLAUDE.md` | No Pocket/Adaptive floor default; `Unbacked` gives no feeds and no raw fallback. |
| R1 seed (2026-10-30) | `session/mutation/toolpath.rs:881-905`; `session/CLAUDE.md` | A tool bind fills only EMPTY or SUGGESTED speed fields under `Speeds`; a Manual value stays; no basis clears the suggested fields ("no underived defaults"). |
| F114 / F042 / F025 | `feeds/CLAUDE.md`; `apply.rs:862-880, 413-480` | `apply` is THE write; `ApplyFeeds` is the one project funnel; raw-result doors are `test-support`; the scope is an enum. |
| Checkpoint I-1 (2026-08-12) | `apply.rs:28-36` | No per-field apply; every apply resolves the whole point first. |
| G-SPEEDSAPPLY (2026-10-12) | `apply.rs:84-105` | A speeds-only apply ships the feed for the geometry the op keeps; geometry passes do not run. |
| T-9 | `apply.rs:121-136` | Feed and plunge round DOWN. |
| Profile boundary | `profile.rs:15-21` (plan §3.2) | The pre-sim profile must not own the post-sim verdict; report 01 (b) fix 4 says a change needs an operator ruling. |
| Export gate | `tool_load/CLAUDE.md`; `verdict.rs:1142` | Only a sourced bound may refuse an export. |
| LUT ingest | `vendor_lut.rs:579-640`; `data/vendor_lut/industrial_only/README.md`; `material/CLAUDE.md` | Rule 1 no degenerate range; rule 2 an `exact` row prints a chip; rule 3 grade A only for vendor charts; rule 4 paired fields; industrial rows stay out of `embedded()`; every species `source_id` in `source_manifest.json`; Spektra re-grade to derived/B (`extrapolation RULINGS.md:428-439`). |
| CREDITS | root `CLAUDE.md` | A new dataset, formula or reference updates `CREDITS.md`. |
| FM1 golden | `feeds/CLAUDE.md` Sentries | A feeds change re-blesses the matrix CSVs in the same commit and names the moved cells. |

---

## 6. Sentries a redesign moves

From `feeds/CLAUDE.md`, `tool_load/CLAUDE.md`, `material/CLAUDE.md`, `session/CLAUDE.md` and
the root-cause reports. All are files under `crates/rs_cam_core/tests/` unless marked.

**Golden and census**

- `feeds_matrix_instrument_fm1` — 1000 cells equal `tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`; the sim subset `_sim.csv` (about 150 s, `heavy-tests`). Any input change moves it.
- `lut_resolver_census_a6`, `lut_resolver_purposes_a7`, `lookup_parity` — Suggest and the gate read one row.
- `every_cell_declares_its_feeds_support_fm0`, `clueless_cells_refuse_and_backed_cells_ship_fm5`, `suggest_refuses_what_the_registry_refuses_fm4`, `micro_extrapolation_refuses_fm9`.
- `every_feeds_formula_constant_states_its_source_f201` — 144 unsourced constants.
- `--lib feeds::suggest::tests::operation_feeds_hints_is_an_explicit_per_op_decision` (`suggest/tests.rs:2544`) — the hint table.

**Geometry and dial**

- `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`, `one_depth_derate_for_feed_and_band_fm3`, `finish_depth_is_reported_not_capped_fm6`, `long_tool_derate_is_shown_ld1`.
- `a_speeds_only_apply_ships_the_feed_for_the_depth_it_keeps_g_speedsapply`, `suggest_feed_matches_final_geometry`.

**Speeds and power**

- `rpm_follows_the_feed_ceiling_fm8` (R4 Q10), `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`, `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`.
- `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`, `a_published_power_is_at_the_depth_that_cuts_g_s2`, `suggest_power_ceiling_after_pass9_g_suggest_powerstale`, `power_ceiling_parity_f2`, `the_power_ladder_pulls_the_right_lever_g_ladder`.
- `rubbing_floor_warns_and_never_lifts`, `a_refused_deflection_is_not_a_zero_g_t4`, `chipload_thinning_magnitude_survey`.

**Chip claims**

- G1: `a_size_claim_states_its_rule_range_and_residual_g1`, `every_consumer_reads_one_claimed_band_g1`, `the_micro_tapered_finish_ships_the_printed_tip_row_g1`, `a_tapered_row_is_read_at_the_tip_a1`, `a_printed_value_is_held_as_a_point_a2`.
- G2: `one_janka_table_for_row_and_query_g2`, `a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`, `the_onsrud_vbit_rows_serve_mdf_and_plywood_g2`.
- G3: `a_printed_cell_serves_every_family_through_one_claim_g3`, `the_printed_tip_row_serves_every_3d_finish_g3v8`, `a_generic_tool_reads_the_standard_row_first_g_series`, `feed_ramp_and_gate_read_one_chart_cell_g_series`.
- B4/B6: `a_vbit_row_is_read_at_its_printed_key_b4`, `the_force_line_is_printed_per_family_b6`, `wood_species_library_provenance`, `efficiency_abstains_without_kc_g_specenergy`.

**Plunge and ramp**

- `a_flat_end_plunge_is_the_side_chip_over_z_g6`, `a_plunge_is_a_named_fraction_of_the_side_feed_g10`, `a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`, `a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10`, `suggest_leaves_the_entry_style_to_the_operator_g10`, `a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`.

**Session and gate**

- `empty_feeds_take_suggest_r1` (R1 seed), `unset_values_refuse_s11`, `predicted_feed_gates_f035`, `a_weak_bound_cannot_refuse_an_export_g_s4weak`, `the_adopt_modulates_as_run_simulation_does_f115`, viz `the_power_row_names_its_force_line_b6`.
- `wanaka_suggest_integration` takes minutes: ask first.

---

## 7. Suggestions (my opinion, not facts)

### 7.1 The root defect in type terms

`FeedsInput` uses `Option<f64>` with "`None` = auto-calculate" (`mod.rs:414-418`). The
calculator then fills `None` from 31 sources (section 1.8) and returns plain `f64`s. The
apply step cannot tell an operator value from a family default, so `Both` overwrites the
first with the second. Provenance is derived afterwards from the row, not from the path, so
it mislabels the geometry (section 3).

The fix is to make "where it came from" part of the value, and to make the missing inputs
explicit requests to the operator, not fills.

### 7.2 A typed cut description in, not hints

Replace `FeedsHints` and the two `Option<f64>` fields with one required input, built by
each op from its own config and heights:

```rust
pub struct CutGeometry {
    pub axial: Axial,      // from the op: depth_per_pass + total depth, or a z-step
    pub radial: Radial,    // Stepover(mm) | Slot | ScallopTarget(mm) | SurfaceFollow
    pub depth_total: Option<TotalDepth>, // top - floor from heights; None only by declaration
}
pub enum Axial { OperatorStep(Mm), ZStep(Mm), CurveDepth(Mm), Unknown(MissingInput) }
```

- Each op states its geometry in `OperationConfig` (an exhaustive match, as
  `capabilities()` is today). Profile and Project Curve state `Radial::Slot` for the first
  level. 3D Rough states `OperatorStep(depth_per_pass)` and `depth_total` from heights.
- `Unknown(MissingInput)` is a value, not a default. The calculator cannot read a number
  out of it. The caller must refuse, ask, or pick a named and cited rule.

### 7.3 Values that carry their source

```rust
pub struct Sourced<T> { value: T, basis: Basis }   // no pub constructor for value alone
pub enum Basis {
    Operator,                      // the op's own field
    Printed(RowRef),               // a vendor row cell
    Claim(ClaimRef),               // G1/G2/G3/G6/G10 claim on a RowRef
    Rule(&'static FormulaConstant),// a named rule; Cited or Unsourced is visible
    Derived { from: Vec<Basis>, law: &'static str },
}
```

- `FeedsResult` holds `Sourced<Rpm>`, `Sourced<Feed>`, `Sourced<Axial>`, `Sourced<Radial>`,
  `Sourced<Plunge>`, `Sourced<Power>`. `FeedsProvenance` then is a projection of
  `basis`, not a second guess (`provenance.rs:352-398` goes away).
- A `Sourced` value is built only by the functions that own its rule (as `resolve_plunge`
  already owns the plunge). The compiler then stops a family default from turning into a
  shipped number without a `Rule` basis that names it.
- The G10 plunge takes `Sourced<Feed>` and checks that `basis` is the same `RowRef` as its
  claim. A feed from an Onsrud row cannot meet an Amana fraction (report 02 (c)).

### 7.4 A gate on what may ship

- Split the apply input into `Recommendation<Complete>` and `Recommendation<Partial>`
  (type-state). `apply` accepts only `Complete`. `Complete` needs every written field to
  have a basis other than an unsourced default, OR an explicit operator acceptance.
- Under every scope, a field with `Basis::Operator` is never replaced by a lower-grade
  basis. `Both` may lower an operator depth (a cap) but never raise it (report 01 fix 4).
- The ten-plus geometry passes become "caps on a sourced value". A pass that moves a value
  returns a new `Sourced` whose basis names the pass and its constant.

### 7.5 One engagement definition

- One `Engagement { ap, psi, d_eng, section }` type with ONE constructor from geometric
  `ae` and ONE from a sim sample (arc). Both feed `PowerTerms::of` and
  `lateral_cutting_force`. This closes the arc-equivalent vs geometric width gap and the
  modulator rectangle (section 4.2). The memory rule "one load definition" asks for this.
- Pre-sim power evaluates an engagement SET, not a point: the nominal, the slot (when the
  op can slot), and the planner's cap (adaptive `trochoid_cap_mult`). Ship the maximum
  with its case name.
- A post-sim `PowerVerdict::Exceeds` becomes an input of a second Suggest stage
  (`Sourced<..>` with `Basis::Simulated`). This needs an operator ruling on the profile
  boundary (`profile.rs:15-18`).

### 7.6 Decisions for the operator

1. R4 Q10 at the default 6000 cap: compare "lower RPM, same chip" with "same RPM, smaller
   chip" by power, or keep Q10 as is.
2. Whether the family default table (`mod.rs:2978-3083`) may ship at all, or becomes a
   preview only with `Unknown` inputs refused (the R1 "refuse if clueless" logic applied
   to geometry).
3. Whether a score tie between rows refuses and names both rows (report 02 F-a3) instead
   of the id order (`vendor_lookup.rs:741`).
4. The order of work: 7.2 and 7.3 first (they move no number by themselves if every op
   keeps its present basis); 7.4 next (it moves FM1 cells, re-bless in the same commit);
   7.5 last (it moves the sim CSV and the gate sentries).
