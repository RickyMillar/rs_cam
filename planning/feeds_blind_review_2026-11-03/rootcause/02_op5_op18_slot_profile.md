# Root cause 02: op 5 "River channel 6 mm" and op 18 "Outline profile with tabs"

Scope: read-only analysis at HEAD `40cf411a7` (2026-11-03). No source edit, no cargo run.
Tool: id 0, 6 mm 2F flat, `cut_direction = "up_cut"`, stick-out 30 mm, White Oak,
`spindle_strategy = "match_chart"`, machine RPM 6000-24000, no `max_cutting_feed_mm_min`.
Suggest output: `suggest/suggest_operator.jsonl` records 5 and 18.

Line numbers are working-tree line numbers. The working tree equals HEAD for each cited file,
except `ops/project_curve.rs`: another session edits that file. Its citations come from
`git show HEAD:crates/rs_cam_core/src/ops/project_curve.rs` and carry the mark "(HEAD)".

Path prefix: `core/` = `crates/rs_cam_core/src/`.

## Summary

| Sub-question | Root cause (one line) | Confidence |
|---|---|---|
| (a) row and size | Project Curve reads (Contour, Finish) and Profile reads (Contour, Roughing). The only 6.35 mm hardwood Contour rows are two Onsrud specialty rows. The id order breaks the tie. The calculator assumes the family default ap/ae, never a full slot. | high |
| (b) rpm 7843 | The Onsrud row prints no RPM, so RPM = 200 m/min / (pi x 6) = 10610. The 0.382 chip at 10610 gives 8116 mm/min, over the 6000 cap. Ruling R4 Q10 lowers RPM to hold the chip. No chip ceiling exists other than the vendor band itself. | high |
| (c) plunge 2999 | G10 plunge = shipped side feed / Z. The side feed comes from the Onsrud row, not from the Amana row that the G6 ramp chip reads. | high |
| (d) no power | The CLI passes `fallback = None`, and Profile and Project Curve have no `stepover()`. | high |
| (e) straight plunges | The Project Curve registry row strips every dressup. The generator emits a straight `EntryPlunge` at the plunge rate at each chain start. | high |

## (a) Why an Onsrud finishing row and a G1 x0.97 size claim

### Symptom

Both ops ship `vendor_source = onsrud-hardwood-60-100mw-1_4-finish` (a compression spiral,
contour, finish, 6.35 mm, band 0.3556-0.4064 mm/tooth). The support reads "extrapolated (G1 size):
x0.97 from the 6.35 mm row". The calculator point is ap 0.36 / ae 0.12 mm (op 5) and
ap 4.8 / ae 1.08 mm (op 18). The real cuts are a full slot (ae = 6 mm). The op 5 slot is up to
10 mm deep, and the op 18 slot is 1.5 mm per level to 25 mm.

### Code path

1. `core/compute/catalog/registry.rs:1346-1347`: Project Curve declares
   `feeds_family: Trace`, `feeds_pass_role: Finish`. `:1384` states `lut_route: LutRoute::ByCutter`.
   `:1364` states `feeds_hints: FeedsHints::NONE`.
2. `core/compute/catalog/registry.rs:172-173`: Profile declares `Contour`, `Roughing`;
   `lut_route: LutRoute::AsStated`; `feeds_hints: FeedsHints::NONE` (`:186`).
3. `core/feeds/suggest.rs:995-996`: `feeds_input_for_operation` reads `operation.feeds_style()`
   and `operation_feeds_hints(operation)`. The hints are all `None` for both ops
   (`core/feeds/suggest/apply.rs:1196`; pinned in `core/feeds/suggest/tests.rs:2568,2582`).
4. `core/feeds/vendor_normalize.rs:82-103` (`lut_query_for`): `ByCutter` calls
   `lut_query_by_cutter` (`:108`). A flat end mill goes to `(Contour, Finish)` (`:113`).
   `AsStated` keeps Profile at `(Contour, Roughing)`.
5. `core/feeds/vendor_lookup.rs:936`: the operation family is a hard filter. The pass role is only
   a score term: +45 on a match, -25 on a miss (`:1036`). No hardwood flat Contour *roughing* row
   exists, so Profile also lands on a Contour *finish* row.
6. The hardwood flat Contour rows in the LUT: `amana-flat-hardwood-contour-3175-2f` (3.175 mm),
   and Onsrud 60-100MW (compression) and 60-200 (downcut) at 3.175-19.05 mm. At 6 mm the two
   6.35 mm Onsrud rows have equal scores. The 3.175 mm Amana row loses about 168 points on the
   diameter term (`:1000-1007`).
7. `core/feeds/vendor_lookup.rs:818` `prefer_series`: the tool is `up_cut`, so it names no
   series. Both Onsrud rows are specialty (`series::series_rank` = 2, `core/feeds/series.rs:231`).
   No standard row ties at 6.35 mm, so the series rule does not act.
8. `core/feeds/vendor_lookup.rs:741` (`beats`): the last tie-break is `candidate_id < best_id`.
   `...60-100mw-...` sorts before `...60-200-...`. Thus the compression row (0.356-0.406) wins
   over the downcut row (0.127-0.178) only on the string order of the id.
9. The G1 size law then scales the 6.35 mm row to 6 mm (x0.97, log-log between the 4.7625 and
   6.35 mm rows). The hardness transfer gives the band 0.3570-0.4080. The chip is the band
   midpoint, 0.38246.
10. `core/feeds/mod.rs:2076` `operation_default_profile(input.operation, input.pass_role)`:
    the calculator uses the op's *declared* family (Trace for Project Curve), not the routed LUT
    family. `(Trace, Finish)` at `:3054` gives ap = 0.06 D = 0.36, ae = 0.02 D = 0.12.
    `(Contour, Roughing)` at `:3007` gives ap = 0.8 D = 4.8, ae = 0.18 D = 1.08. Neither op
    supplies an override (`:2103-2108`).
11. `core/feeds/suggest/axial_envelope.rs:213-214`: the Project Curve axial envelope
    (`CurveRough`) assumes `radial_woc = 0.2 x D`, not a slot. The depth check at `:317` compares
    `cfg.depth` (a surface offset, here `-4.0`) with the safe DOC. A negative offset never fires
    `ProjectCurveDepthInfeasible`.

### Root cause

No product path tells Suggest that these ops cut a full slot. `FeedsHints::NONE` and the family
default table give a light finish or contour engagement. The LUT holds only industrial Onsrud
Contour rows near 6 mm, and the id string order picks the compression row with the highest chip.

Full slot (ae = 100 % D) is never seen. No op sets `FeedsHints::radial_width_mm`
(`core/compute/catalog.rs:1920`: "none of the current ops set this").

### Evidence

```
"axial_depth_mm": 0.36, "radial_width_mm": 0.12        (op 5, calculator)
"axial_depth_mm": 4.8,  "radial_width_mm": 1.08        (op 18, calculator)
onsrud-hardwood-60-100mw-1_4-finish  flat_end contour finish 6.35 2  0.3556 0.4064  (compression)
onsrud-hardwood-60-200-1_4-finish    flat_end contour finish 6.35 2  0.127  0.1778  (downcut)
```

### Fix options

- F-a1: give Profile and Project Curve a `FeedsHints::radial_width_mm = D` (slot) on the first
  level, and an axial hint from the cut depth. For Project Curve, read the depth below the stock
  top, not `cfg.depth`. Blast radius: `FeedsHints` is read only through `operation_feeds_hints`
  (1 production caller, `core/feeds/suggest.rs:996`). The per-op pin test
  `operation_feeds_hints_is_an_explicit_per_op_decision` must change on purpose. The golden
  matrix `feeds_matrix_instrument_fm1` moves: the CSV has 40 Project Curve cells and the
  Profile cells.
- F-a2: add a slot or row-family term to the lookup. Option: the chip of a slot reads a
  (Pocket, Roughing) row, as the G-DCFLAT route does for a flat Drop Cutter.
  `lut_query_for` has 2 production callers (`core/feeds/vendor_normalize.rs`,
  `core/tool_load/chipload.rs`). The census `lut_resolver_census_a6` holds them to one row.
- F-a3: replace the id tie-break between specialty rows at one size. Option: prefer the lower
  band, or refuse the tie and name both rows. Blast radius: `beats` is used by `best_by_score`
  only, but every lookup that ends in a tie moves (re-bless FM1).
- F-a4: change the Project Curve envelope to the slot WOC, and check the true cut depth.

Sentries: `lut_resolver_census_a6`, `lookup_parity`, `a_generic_tool_reads_the_standard_row_first_g_series`,
`a_size_claim_states_its_rule_range_and_residual_g1`, `feeds_matrix_instrument_fm1`,
`every_op_states_its_capabilities_w2_4`, `suggest_feed_matches_final_geometry`.

Confidence: high. The tie-break claim is from code reading. I did not run a lookup.

## (b) Why the RPM drops to 7843 and the feed stays at the cap

### Symptom

RPM 10610 goes to 7843, and the feed is 5999. The chip stays at 0.382 mm/tooth. At the operator's
10610 RPM and the cap, the chip is 0.283. At 18000 RPM and the cap, the chip is 0.167.

### Code path

1. `core/feeds/mod.rs:1690-1703` Step 1: `ideal_rpm = base_cutting_speed x 1000 / (pi x effective_d)`.
   Solid wood speed = 200 m/min (`core/material/mod.rs:735`, marked `unsourced`).
   200 000 / (pi x 6) = 10610.
2. `core/feeds/mod.rs:1944`: a vendor RPM replaces it only when the row prints one. The Onsrud
   row prints no `rpm_nominal`, so 10610 stays. `MatchChart` makes no other change.
3. The feed = 0.38246 x 2 x 10610 = 8116 mm/min.
4. `core/feeds/mod.rs:2603` Step 7: the ceiling is `cutting_feed_ceiling_mm_min()` =
   `min(max_cutting_feed, DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000)` (`core/machine/mod.rs:628,665`).
5. `core/feeds/mod.rs:2634`: `target = floor(6000 / (8116/10610)) = floor(7843.9) = 7843`.
   Then the RPM is 7843 and `feed = chip_per_rev x rpm` = 5999.3. The code pushes
   `RpmLoweredForFeedCeiling`. The power check (`fits_power`) reads the calculator ap/ae
   (0.36/0.12), so it passes at 0.0006 kW.

### Root cause

Ruling R4 Q10 (`core/feeds/mod.rs:2589-2600`) holds the chip on the constant-chip line and moves the
RPM down. That is correct only if the chip is correct. The chip is the Onsrud industrial
compression band (sub-question a). The only chip ceiling is that band (`chipload_bounds_mm`
0.357-0.408). The force, power and deflection checks all read the light default engagement, so
no check sees a 10 mm full slot. The R4 rule forbids an L/D feed factor, so
`LongToolDerate 0.88` does not cut the chip.

### Evidence

```
"RpmLoweredForFeedCeiling { rpm_from: 10610.33, rpm_to: 7843.0, feed_ceiling_mm_min: 6000.0,
  rpm_floor: 6000.0, floor_source: MachineMinimum, held: true }"
"chip_load_mm": 0.38246494656954033, "chipload_bounds_mm": [0.35697, 0.40796]
```

### Fix options

- Fix the chip at its source (F-a1 to F-a3). Then 0.382 does not reach Step 7.
- F-b1: an absolute chip ceiling per diameter and family for a slot, from a printed slot row
  (Amana pocket hardwood 6 mm). Blast radius: Step 7 and the modulator. Sentries:
  `rpm_follows_the_feed_ceiling_fm8`, `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`,
  `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`, `feeds_matrix_instrument_fm1`.
- F-b2: in Step 7, check the force and power at the real engagement. This needs F-a1 first.

Confidence: high. The numbers reproduce from the code by hand.

## (c) Why the plunge is 2999 mm/min

### Symptom

The plunge is 2999 = 5999 / 2. At 7843 RPM that is 0.191 mm/tooth axial. The G6 ramp basis on the
same card states 0.0656 mm/tooth (Amana ramp-down). The plunge is 2.9 x that value.

### Code path

1. `core/feeds/mod.rs:2695`: `plunge::plunge_basis(input, ...)`.
2. `core/feeds/plunge.rs:258-298`: an unrouted query; `plunge_rule` (G10) gives a claim for a
   flat end mill of 3.0-12.7 mm, 2-3 flutes, in wood.
3. `core/feeds/extrapolation/plunge.rs:631`: `PlungeFraction::OneOverZ => 1.0 / flutes` = 0.5.
4. `core/feeds/plunge.rs:308`: `rule = claim.fraction x feed_mm_min`, a fraction of the
   *shipped side feed*.
5. `core/feeds/suggest/apply.rs:265`: the funnel runs `resolve_plunge` again at the final feed
   (5999) and writes 2999.
6. The G6 ramp chip (`FeedsResult::ramp`) comes from the drill rule on
   `amana-flat-hardwood-pocket-6000-2f-spektra` (side chip / Z = 0.0656). It feeds only
   `resolve_ramp_feed` (`core/feeds/suggest/apply.rs:329`). Op 5 has entry `none`, so the
   ramp feed falls back to the plunge.

### Root cause

G10 encodes Amana's rule "Ramp Down = feed / flutes" as a fraction of whatever side feed ships.
The rule is true only when the side feed is from the same Amana cell. Here the side feed is from
the Onsrud compression row, so the axial chip is 0.382 / 2 = 0.191, not 0.0656. The card notes
this itself: "the G6 vertical rate is 1028 mm/min and the plunge (feed / Z, ruling Q4) is
2999 mm/min; the two agree only at depth tier 1" (record 18).

### Fix options

- F-c1: bound the plunge by the G6 axial chip when a G6 claim exists:
  `plunge = min(F / Z, ramp_chip x rpm x Z)`. That gives 1028 mm/min at 7843 RPM.
  Blast radius: `resolve_plunge` has 3 production callers (`core/feeds/mod.rs`,
  `core/feeds/suggest/apply.rs`, `core/feeds/provenance.rs`). Ruling Q4 must change,
  because it is an operator ruling.
- F-c2: fix the side feed (F-a1 to F-a3). Then F / Z falls with it.

Sentries: `a_plunge_is_a_named_fraction_of_the_side_feed_g10`, `a_flat_end_plunge_is_the_side_chip_over_z_g6`,
`feed_ramp_and_gate_read_one_chart_cell_g_series` (it compares the ramp only where the feed has the
(Pocket, Roughing) home, so it does not see this case),
`a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10`, `feeds_matrix_instrument_fm1`.

Confidence: high.

## (d) Why no power figure exists

### Symptom

`"shipped_power": {"unmodeled": "NoRadialEngagement"}` for both ops.

### Code path

1. `crates/rs_cam_cli/src/suggest.rs:51-56`: `power_at_operating_point(shipped, tool, material,
   machine, None)`; the fallback is `None`.
2. `core/feeds/operating_point.rs:294-301`: `ae = operation.stepover()`, else the fallback.
   `ProfileConfig` and `ProjectCurveConfig` have no stepover (`"stepover": null` in the jsonl).
   With no fallback the code returns `Err(PowerUnmodeled::NoRadialEngagement)`.
3. The GUI card (`crates/rs_cam_viz/src/ui/feeds/compare.rs:860-867`) passes the calculator point
   as the fallback. It shows a figure at ae 0.12 / ap 0.36 mm (op 5): 0.0006 kW. That figure is
   wrong for a full slot.

### Root cause

A contour or curve op has no stepover field. The CLI passes no fallback, so the power is
refused. The GUI fallback is the light default engagement of (a), so the GUI number is
misleading too. Neither surface uses ae = D for the slot.

### Fix options

- F-d1: give the slot engagement through F-a1. Then both surfaces get a real figure.
- F-d2: pass the same fallback in the CLI as the GUI does (parity). This shows a number, but it
  is the wrong number until F-a1. Blast radius: `crates/rs_cam_cli/src/suggest.rs` only
  (this file is untracked).

Sentries: `a_published_power_is_at_the_depth_that_cuts_g_s2` (it pins
`NoRadialEngagement` for a drill, `:565,590`), `the_power_row_names_its_force_line_b6` (viz).

Confidence: high.

## (e) Why op 5 has 168 straight plunges up to 10 mm

### Symptom

`compare.json` gate: 168 entries, all `entry_plunge`. The tool descends straight up to 10 mm at
the plunge rate.

### Code path

1. `core/compute/catalog/registry.rs:1354-1356`: `dressup_policy: DressupPolicy::strip_all(
   "Incompatible with Project Curve: each ring would get a phantom diagonal cut.")`.
2. `core/compute/catalog/schema.rs:647`: `strip_all` sets `strip_all_reason`.
3. `core/compute/config.rs:1692-1704` (`normalize_for_op`): the code sets `entry_style = None`,
   `lead_in_out = None` and `link_moves = None`. The project already has `entry_style = "none"`.
4. `core/ops/project_curve.rs:369-371` (HEAD): the code sets the chain Z to `tip_z - params.depth`
   (`depth = -4.0`: 4 mm above the projected surface model 4).
5. `core/ops/project_curve.rs:379,393` (HEAD): each chain calls `emit_path_segment_with_intent`.
6. `core/toolpath.rs:428-429`: rapid to `safe_z`, then `feed_to(path[0], plunge_rate,
   MoveIntent::EntryPlunge)`. This is a vertical feed to the full chain depth.
7. `capabilities.ramp_descent = false` (`registry.rs`, Project Curve row). The op has no depth
   per pass (`DocBasis::CurveOnSurface`, `:1366`), so each chain is one full-depth level.

### Root cause

The registry strips every entry dressup from Project Curve. The reason is that a ramp or lead on
a multi-ring DXF cuts a phantom diagonal. No in-op entry replaces the dressup, so the generic
segment emitter plunges straight at every chain start. That plunge is at the plunge rate from (c).

### Fix options

- F-e1: an in-op ramp along the chain itself: descend along the first part of the chain, then
  cut the chain. This has no phantom diagonal, because the ramp stays on the curve.
  Blast radius: `emit_path_segment_with_intent` has callers in 16 files, so add a new emitter,
  not a change to the shared one.
- F-e2: depth levels for Project Curve (the reviewer note: levels of 5 mm or less).
- F-e3: until F-e1 exists, make Suggest write a plunge from the G6 axial chip for an op that
  strips entries (see F-c1).

Sentries: `dressup_policy_table_is_pinned` (`core/compute/config.rs:2116`),
`finishing_defaults_have_no_ramp_entry_r10`, `every_op_states_its_capabilities_w2_4`,
`suggest_leaves_the_entry_style_to_the_operator_g10`, and the unit tests in
`core/ops/project_curve.rs`.

Confidence: high.
