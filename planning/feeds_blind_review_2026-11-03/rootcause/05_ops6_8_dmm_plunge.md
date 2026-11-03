# 05 Ops 6, 7, 8: the DMM 2 mm single flute and its entries

Topic: the pocket ops 6, 7 and 8 with tool id 5, DMM DSP1f-D2.0-L08 (2 mm, 1 flute, centre
cutting not known). Read-only root-cause analysis, 2026-11-03. No source file was changed.

Binary: `target/release/rs_cam_cli`, built 2026-11-03 09:03, after HEAD `40cf411a7` (08:41).
All runs: `rough-score --no-sim` under `systemd-run ... MemoryMax=8G`, on scratch copies in
`rootcause/scratch_ops6_8/` (`wanaka21_ops6_8.toml` and the `v_*.toml` variants that
`make_variants.py` writes; one dressup field changes in one toolpath block). Each run took 4-10 s.
Outputs: `scratch_ops6_8/<name>.json` and `.err`; scripts `run.sh`, `run2.sh`, `run3.sh`.

`--set` cannot change a dressup field (`entry_style`, `ramp_angle`, `entry_clearance_mm`):
the CLI refuses it ("unknown parameter 'entry_style' for Pocket operation"). The variants therefore
change the scratch TOML.

## Evidence table (op 8 unless stated)

| Run | Change | Moves | Entry moves | Entry runs by first intent |
|---|---|---|---|---|
| op8_base | none (ramp 3 deg, clearance 0.5) | 640 | 409 | plunge 9, ramp 1 |
| op8_none | `entry_style = "none"` | 232 | 10 | plunge 10 |
| op8_clear0 | `entry_clearance_mm = 0` | 641 | 410 | plunge 10 |
| op8_clear2 | `entry_clearance_mm = 2` | 1335 | 1104 | plunge 6, ramp 4 |
| op8_ramp1 | `ramp_angle = 1` | 1820 | 1589 | plunge 9, ramp 1 |
| op8_ramp10 | `ramp_angle = 10` | 640 | 409 | plunge 9, ramp 1 |
| op8_dpp5 | `--set 8.depth_per_pass=5` (one level) | 296 | 272 | ramp 1 |
| op8_suggest | `--set 8.depth_per_pass=0.3333 --set 8.stepover=0.47` | 1616 | 911 | plunge 47, ramp 1 |
| op6_base | none (entry none) | 31511 | 570 | plunge 570 |
| op6_ramp | `entry_style = "ramp"` | 75374 | 44403 | plunge 271, ramp 299 |
| op7_base | none (entry none) | 194 | 16 | plunge 16 |
| op7_ramp | `entry_style = "ramp"` | 3000 | 2807 | plunge 15, ramp 1 |

`rough-score --no-sim` writes no simulation: `collision_count` and `rapid_collision_count` are 0
in every row because no simulation ran. They are not evidence of a safe path.

## (a) Op 8: the ramp dressup ramps every level; the counter names 9 of them "plunge"

**Symptom.** `compare.json` gate_checks and `compare.md` line 110 say: 10 entries, 9 straight
plunges, 1 ramp. The review reads this as "the ramp dressup ramps only one entry".

**Code path.**

1. The Pocket emits each ring as rapid to `safe_z` → `EntryPlunge` feed → ring → `Retract`
   (`ops/pocket.rs:364-398` → `Toolpath::emit_closed_contour_with_intent`,
   `toolpath.rs:472-495`). Every ring of every level is a new entry from the retract height.
   No step-down inside a pocket exists in the IR: op 8 has 1 ring per level, so 10 entries and
   10 retracts (`op8_none`: 10 entry moves, 10 retract runs).
2. `apply_entry` (`dressup/mod.rs:384-552`) replaces EVERY vertical fed descent of more than
   0.1 mm (`is_plunge`, `dressup/mod.rs:1107-1118`). It has no "first entry only" rule and no
   rapid/safe-Z rule.
3. `emit_ramp` (`dressup/entry_descent.rs:520-722`) calls `rapid_to_entry_top`
   (`entry_descent.rs:436-478`). That function rapids to `rapid_floor` =
   `material_top + ENTRY_CONTACT_CLEARANCE` (conservative top, sliver-safe disc read), then FEEDS
   straight down to `contact` = `contact_top + entry_clearance_mm` (cell-centre disc read) with
   the intent `MoveIntent::EntryPlunge` (`entry_descent.rs:468-476`). Then the fold ramp follows
   as `MoveIntent::EntryRamp` moves (`entry_descent.rs:686-696`).
4. `rough-score` counts a run of entry moves and names the run by the intent of its FIRST move
   (`crates/rs_cam_cli/src/rough_score.rs:453-461`, `count_moves` at `:432`).

**Root cause.** The engine ramps all 10 levels. On levels 2-10 the op's own replayed stock reads
the uncut pocket wall in the conservative disc (25.5) but the cleared floor in the cell-centre
disc, so each ramp starts with a short straight `EntryPlunge` feed through the open pocket.
`count_moves` labels such a run by that first move, so 9 ramps show as `entry_plunge`.

**Evidence.**

- `entry_style = none` gives 10 entry moves (1 per level). The ramp adds 399 entry moves, about
  40 per level. A 1 mm ramp drop (0.5 mm level + 0.5 mm clearance) folded along the ring needs
  about that many moves; one ramp of 399 moves with 9 single-move plunges does not fit
  `op8_dpp5`, where a 5.5 mm drop in one ramp takes 272 moves.
- `entry_clearance_mm = 0`: `contact` falls 0.5 mm under `rapid_floor` on every level, level 1
  included. All 10 runs then start with `EntryPlunge`, and the entry moves rise by exactly 1
  (409 → 410). Only level 1 gained the lead feed; levels 2-10 had it already.
- `entry_clearance_mm = 2`: the lead feed exists only where
  `wall top + 0.5 > floor_(k-1) + 2`, that is, where the cut depth before level k is more than
  1.5 mm: levels 5-10. Prediction 6 "plunge" + 4 "ramp"; measured 6 + 4.
- `ramp_angle = 1` triples the entry moves (409 → 1589) and keeps the 9/1 split: every level
  ramps, and the angle is read (`dressup_apply.rs:763`). (The 10 deg run gives the same 409 moves
  as 3 deg; I did not explain that equality. It does not change the conclusion.)

**Side findings on the same path (not the 9/1 split).**

- The lead feed at levels 2-10 goes through air (the column under the tool was cut at the level
  before). Its rate is `min(op plunge, entry_plunge_rate)`. `entry_plunge_rate` is half the first
  linear feed of the toolpath (`dressup_apply.rs:713-722`), here the 240 plunge, so 120 mm/min.
  At most 4.5 mm per level: a few seconds, not a safety issue.
- Op 8 has `ramp_feed_rate = 1258` (the cut feed). The ramp rides the full side feed in a near
  slot; its vertical rate is 1258 x tan 3 deg = 66 mm/min.

**Fix options.**

1. Fix the counter (recommended). Name a run `entry_ramp` or `entry_helix` when ANY move of the
   run has that intent, or report `entry_runs_by_kind` per move intent plus a separate count of
   "runs with no ramp or helix move". Blast radius: `count_moves` and `MoveCounts` in
   `rs_cam_cli/src/rough_score.rs` only (`rg "entry_runs_by_kind|count_moves"`: 6 lines, one
   file). No test pins the field. The review's `compare.json`, `compare.md` line 110 and the op 8
   risk rows ("9 straight plunges") need a correction after the fix.
2. Tag the lead feed of a ramp or helix as `Linking` (air) or a new `EntryAir` intent instead of
   `EntryPlunge`. Blast radius larger: `rapid_to_entry_top` serves the ramp, the helix and the
   stock-aware plunge (`entry_descent.rs:497`, `:640`, helix door); `EntryPlunge` is read by the
   feed modulator plunge skip, the descent split (`dressup/mod.rs:861`), the plunge-class gate and
   the G-CENTRECUT gate. Not recommended for this symptom.

**Sentries.** `cargo test -p rs_cam_core -q --test entry_moves_stock_aware_g_rampterrain --test
an_entry_ramp_follows_the_arc_g_rampchord --test ramp_contained_in_region_g_rampcontain --test
dressup_span_invariants --test a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`;
`--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut`. For option 1: `cargo test -p
rs_cam_cli -q` (`rough_score_publishes_the_gui_numbers`); add a counter test with a ramp that
starts with a lead `EntryPlunge`.

**Confidence.** High for the classification artefact (three predicted counts matched). Medium
that the lead feed reads the wall top (inferred from the 6/4 split; the moves were not dumped,
because the allowed command prints counts only).

## (b) Ops 6 and 7: no entry, and no check for a tool of unknown centre cutting

**Symptom.** Ops 6 and 7 plunge straight at every ring of every level: op 6 has 570 entries
(18 levels x 8 lakes, about 4 rings each), op 7 has 16. Nothing warns that the DMM plunges.

**Code path.**

- The default IS a ramp for a roughing op: `DressupConfig::for_role(Roughing)` sets
  `entry_style: Ramp` (`compute/config.rs:1599-1620`). `DressupConfig::default()` (the serde
  default) is `None` (`compute/config.rs:1528-1535`).
- The project sets `entry_style = "none"` on toolpaths 12 and 13
  (`nz_wanaka_21_operator.toml` lines 1093 and 1188). `planning/rivmap_block_job.md:698-701`
  records why: G-POCKETRAMP, 21 rapid collisions in the large lakes with the ramp; the plain
  plunge removes them.
- Centre cutting is binary. `ToolConfig::non_centre_cutting: Option<NonCentreCuttingGeometry>`,
  doc "`None` means centre cutting" (`compute/tool_config.rs:478-480`). There is no "not known"
  state.
- The only engine reaction is in `effective_entry_style` (`compute/execute/dressup_apply.rs:
  647-683`): entry None becomes Ramp only when `cutter.non_centre_cutting().is_some()`
  (`:671-680`). The entry builders and the simulation gate also read only `Some` limits
  (`dressup/mod.rs:225-229`, `stock/collision.rs:714-760`).
- The plunge checks do not cover this case: `tool_load/plunge_stress.rs:28-48` caps only ball
  and tapered-ball tips (`None` for a flat end mill); `geom.plunge_exceeds_feed`
  (`diagnostics/adapters/from_static_checks.rs:554-575`) compares rates only; the Q12 caution
  (`feeds/ramp.rs:787-798`, `feeds/plunge.rs:43`) is about the cut direction, not the centre.

**Tool data.** The DMM entry (`nz_wanaka_21_operator.toml:216-235`) has no
`[tools.non_centre_cutting]` table, an empty `vendor` and an empty `product_id`. The only tool in
the project with the table is id 6, the 18 mm (core 8.0, protrusion 1.0, line 258). The tool
library (`~/.config/rs_cam/tools/*.toml`) has no DMM tool and no centre-cutting field in any
tool. No vendor LUT row names DMM.

**Root cause.** Ops 6 and 7 have no entry because the operator turned it off to avoid
G-POCKETRAMP. No check warns because rs_cam reads "no `non_centre_cutting` data" as "centre
cutting", so a tool of unknown centre cutting is treated as a safe plunger.

**Evidence.** `op6_base` and `op7_base`: every entry run is `entry_plunge`, 1 move each
(570 and 16). With `entry_style = "ramp"` op 6 gets 44403 entry moves (ramps on all 570 entries,
271 with a lead air feed as in (a)). G-POCKETRAMP needs a simulation to show; it was not run.

**Fix options.**

1. A tri-state centre-cutting field, for example `centre_cutting = "yes" | "no" | "unknown"`
   beside the measured geometry, with "unknown" as the default for a new tool and a static
   finding "a tool of unknown centre cutting plunges N times" when the effective entry is None.
   Blast radius: `rg -c non_centre_cutting` gives 21 files (core `tool_config.rs` 20 refs,
   `tool/mod.rs` 8, `compute/simulate.rs` 8, `rs_cam_cli/src/job.rs` 10, viz
   `ui/properties/tool.rs` 5, `rs_cam_mcp/src/server.rs` 5, project and library loaders). A new
   serde field with a default keeps old files loading; the F118 reachability sentry needs the
   new field on every door.
2. A smaller first step: a static diagnostic for a flat end mill with entry None in a roughing
   op, without a new tool field. It cannot tell a centre-cutting tool from an unknown one.
3. Fix G-POCKETRAMP, so that the operator can turn the ramp back on for ops 6 and 7. The likely
   area is the op's own stock replay (`OwnStockReplay`, `dressup/mod.rs:~250-362`) and
   `rapid_to_entry_top` (`entry_descent.rs:436-478`). Not studied here.

**Sentries.** `--test a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`,
`--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut`, `--test
every_stock_and_tool_field_is_reachable_f118`, `--test
a_tool_outside_its_domain_is_refused_at_every_door_f029`, `--test
finishing_defaults_have_no_ramp_entry_r10`, `--test plunge_guard_p3`.

**Confidence.** High.

## (c) The Suggest values for ops 6, 7, 8

Source: `suggest/suggest_operator.jsonl` (toolpath 12; 13 and 14 are the same tool and row).
Each number below is reproduced to the printed digits.

| Value | Suggest | Pass and code | Arithmetic |
|---|---|---|---|
| chip fz | 0.06994 mm | G1 form A, log-log between two adjacent printed sizes (`feeds/extrapolation/size.rs:1-35`, `bracket` `:167`, `interpolated_claim` `:180`); then G2 Janka law `(row / query)^0.5` (`feeds/extrapolation/hardness.rs:6`, `:310`) | rows 1.5 mm 0.0508 and 3.0 mm 0.1016 (Amana Spektra 2F, `data/vendor_lut/observations/amana_flat_end.json:2301-2325`): slope 1, so 0.0508 x (2/1.5) = 0.06773 ("x1.33"); x (1450/1360)^0.5 = 1.0325 (row Janka proxy 1450, white oak 1360, `material/mod.rs:153`) gives 0.0699386 |
| rpm | 18000 | `SpindleStrategy::MatchChart` uses the row's `rpm_nominal` (`feeds/mod.rs:335-362`) | row `rpm_nominal = 18000` |
| feed | 1258 | fz x rpm x Z | 0.0699386 x 18000 x 1 = 1258.9, rounded down |
| plunge | 240 | G10 flat rule covers 3.0-12.7 mm only, so the material base (`feeds/plunge.rs:33`, `resolve_plunge` `:306`) | 1000 x clamp(2/6, 0.25, 3) / (1360/600)^0.4 = 240.3 |
| depth start | 1.4 → 0.5 | Pocket roughing profile `ap_factor 0.70` (`feeds/mod.rs:2994-2996`); rigidity cap `doc_roughing_factor 0.25` x D (`feeds/suggest/invariants.rs:505-524`, `machine/mod.rs:143-163`) | 0.7 x 2 = 1.4; 0.25 x 2 = 0.5 |
| stepover start | 0.7 | `ae_factor 0.35` (`feeds/mod.rs:2996`) | 0.35 x 2 |
| depth, stepover | 0.335, 0.47 | pass 6b `apply_aggressiveness` (`feeds/suggest/aggressiveness.rs:146-375`): target `k x ld` = 0.85 x 0.75 (L/D 10 > 6, `feeds/mod.rs:1618-1645`); one common scale bisected on force and power, values rounded down to 0.001 mm | scale about 0.671: 0.7 → 0.47, 0.5 → 0.335 |
| depth op 8 | 0.3333 | the same pass snaps the depth to the ladder (`ops::depth::realised_step_down`, `ops/depth.rs:150`) when the op has a total depth | 5.0 / ceil(5.0 / 0.335) = 5 / 15. Ops 6 and 7 (floor by Z, manual top) have no total, so 0.335 ships unsnapped |

**Findings.**

- The chip comes from a 2-flute Spektra chart and goes to a 1-flute tool unchanged per tooth.
  The series filter matches the ANCHOR's flutes (`size.rs:20-24`); the single-flute series mark
  is only a tie-breaker (`feeds/CLAUDE.md`, G3 rulings 2026-10-31).
- The long-tool factor (L/D 10) lowers depth and stepover, never the chip: "no machine or L/D
  feed factor" (`feeds/CLAUDE.md`, R4) and `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`.
  This is why Suggest and the operator both ship fz 0.0699 while the reviewer asks for
  0.030-0.045.
- The L/D share is a step: 0.75 for any L/D over 6 (`LD_SEVERE_THRESHOLD`); L/D 10 gets no more
  derate than L/D 6.1.
- The aggressiveness pass scales the stepover to cut the load, but a contour pocket's first ring
  at each level is a full slot (op 8: the only ring). The stepover lever does not change the slot
  load. Suggest's lower stepover only adds rings (time), which the review classes "low, risky
  (time)".
- Suggest writes no entry style (G10 Q11). Its ramp-feed note for ops 6 and 7 states "the entry
  is a straight plunge" but gives no centre-cutting caution (see (b)).

**Fix options.** These are model rulings, not defects. Candidates for the operator: (1) a
flute-count transfer for a 1F tool read from a 2F row; (2) an L/D chip derate (a ruling change
of R4; blast radius: the FM1 golden matrix
`tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv` re-blesses); (3) a slot-aware
base engagement for the first ring of a contour pocket.

**Sentries.** `a_size_claim_states_its_rule_range_and_residual_g1`,
`every_consumer_reads_one_claimed_band_g1`, `a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`,
`a_plunge_is_a_named_fraction_of_the_side_feed_g10`, `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`,
`long_tool_derate_is_shown_ld1`, `a_generic_tool_reads_the_standard_row_first_g_series`,
`suggest_leaves_the_entry_style_to_the_operator_g10`, `feeds_matrix_instrument_fm1`.

**Confidence.** High (each number reproduced).

## (d) A rest-aware entry: start each level in an opening an earlier op made

**What exists.**

- The machined stock of earlier ops reaches an op only when its `stock_source` reads machined
  stock (`FromRemainingStock`). `GenContext::prior_stock` is `None` for a `Fresh` op
  (`session/compute.rs:247-255`, `generator_seed_stock` `:286-290`). Ops 6, 7 and 8 are `fresh`.
- With a prior stock, two passes are already rest-aware in Z, not in XY:
  1. the entry dressup replay `EntryStockReplay { cutter, prior: ctx.prior_stock }`
     (`dressup_apply.rs:701-707`), which makes `rapid_to_entry_top` rapid through air and treat
     an open column as `air` (straight descent, no ramp, no refusal; `entry_descent.rs:452-478`);
  2. the descent split `optimize_entry_descents_annotated` (`session/compute.rs:1166-1176`,
     `dressup/mod.rs:762`), which rapids to the conservative stock top + 2 mm
     (`dressup/mod.rs:895-899`).
- No pass moves the entry XY. The Pocket starts each ring at its first vertex
  (`ops/pocket.rs:372-395`).

**Consequence on the fresh ops today.** The descent split falls back to the fresh stock top
(25.5) + 2 = 27.5 (`dressup/mod.rs:898-899`), so each entry of op 6 FEEDS from 27.5 down to its
level (14.46 to 5.91) at the 240 plunge, although op 2 opened the lakes. Estimate, not measured:
570 entries x about 17 mm / 240 mm/min = about 40 min of fed air in op 6. A simulation or a
cycle-time print is needed to confirm.

**Where a rest-aware entry would go.**

1. Z only, no new code: set ops 6 and 7 to `from_remaining_stock`. The replay then sees the op 2
   opening; a plunge into an open column is air. Cost: a Pocket from remaining stock is the R3
   rest pass (it cuts only what remains), it needs the ancestor simulation, and
   `rivmap_block_job.md:696` records 7.21 h with the remaining-stock lake pass against 5.81 h now.
2. XY: rotate each closed ring so that it starts where the prior stock top is at or under the
   level Z (or lowest). Two places can hold it:
   - in the Pocket emitter (`pocket_contours_to_sided_toolpath`, `ops/pocket.rs:364`), with the
     seed stock passed in through the layered runner (`compute/execute/layered.rs`,
     `run_layered` `:550`) — the generator already gets `generator_seed_stock` on a rest op;
   - or as a dressup step before `RampEntry` in `DRESSUP_PIPELINE`
     (`compute/execute/dressup_apply.rs:352`), reading `ctx.prior_stock`. A ring rotation moves
     the plunge target, so `Produced::closed_passes` (keyed by the plunge target, `toolpath.rs:
     466-470`) and the lead side (`dressup/lead_side.rs`) must follow it, and the step must keep
     the C1 remap. The dressup CLAUDE.md forbids a pass outside the pipeline, so the dressup form
     must be a new `DressupStep`.
   Either way the rule must run only on a rest op (prior stock `Some`); on a fresh op there is no
   opening to find.

**Blast radius.** `DRESSUP_PIPELINE` has 12 steps and a test that pins their order
(`dressup_apply.rs:1196-1230`). `pocket_contours_to_sided_toolpath` is the shared Pocket emitter
(the rest pass reuses it). The golden `every_op_keeps_its_toolpath_g_opgolden` moves for any
Pocket.

**Sentries.** `--test capability_link_moves_safety`, `--test dressup_span_invariants`, `--test
a_lead_curls_away_from_the_part_p6_0c`, `--test
a_peck_ladder_is_never_split_into_a_rapid_into_stock_g_pecksplit`, `--test
isoclip_entry_ramp_g_isoclipentry`, `--test entry_moves_stock_aware_g_rampterrain`, and the op
golden `every_op_keeps_its_toolpath_g_opgolden`.

**Confidence.** High for "no XY rest-aware entry exists" and for the fresh-stock fallback.
Low-medium for the 40 min estimate.

## Corrections for the review

- `compare.md` line 110, `compare.json` gate_checks op 8, and the op 8 `ramp_angle` risk rows:
  "9 straight plunges, 1 ramp" is a counter artefact. All 10 levels ramp at 3 deg; levels 2-10
  begin with a short fed descent through the open pocket.
- The op 6/7 risk "a straight 0.5 mm plunge per level into fresh oak" stands, but each RING
  plunges, not each level (570 entries in op 6), and the upper levels plunge where op 2 already
  opened the lake (the op is `fresh`, so the engine does not know it).
