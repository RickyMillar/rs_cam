# Root cause 06: the risky Suggest items of the V-bits (ops 9, 10, 13), the R1 tapered ball (ops 16, 17) and op 15

Scope: read-only analysis at HEAD `40cf411a7` (2026-11-03). No source edit, no cargo run, no
rough-score run (the Suggest records and `compare.md` hold the numbers that this note needs).
Inputs: `compare.md`, `compare.json`, `review_B.md`, `review_C.md`, `review_A.md` (op 15),
`suggest/suggest_operator.jsonl` records 9, 10, 13, 15, 16, 17, and the operator project
`planning/rivmap_block_job/nz_wanaka_21_operator.toml` (read only).

Path prefix: `core/` = `crates/rs_cam_core/src/`. Line numbers are working-tree line numbers.

Project facts that the analysis uses: `spindle_strategy = "match_chart"`, `aggressiveness = 0.85`,
`[job.machine.rigidity] adaptive_doc_factor = 2.0`, `adaptive_woc_factor = 0.2`, White Oak.

## Summary

| Item | Root cause (one line) | Confidence |
|---|---|---|
| (a1) op 9 fz 0.127 (reviewers 0.035-0.057) | Suggest ships the midpoint of the Amana 18 deg 1F band at an assumed depth of 0.06 x D. The real 6.42 mm depth (2.02 x D) never reaches the depth de-rate, and the long-tool share has no effect on a finish op. | high |
| (a2) V-bit flat tip | `ToolConfig` has no tip field. `geometry_hint` and `build_cutter` make every V-bit sharp. For the feeds this changes nothing (B4 keys the row at the angle; the row prints the rpm). It changes the cut geometry by 0.85 mm of Z on the V20. | high |
| (a3) op 10 dpp 0.371, op 13 dpp 0.3 | Trace carries no axial hint, so the calculator uses the Trace/Finish default ap = 0.06 x D = 0.381 mm. Apply writes it as depth per pass and snaps it to 14 (op 10) or 2 (op 13) equal passes. | high |
| (a4) op 10/13 rpm 18000, feed 2743 | The AMS-159 60 deg row prints 18000 rpm and one point 0.0762 mm. `match_chart` keeps the chart rpm. Reviewers agree for op 10 (not risky). | high |
| (a5) V plunge rubs (381, 914) | The plunge rule is a fraction of the side feed (60 deg) or the material base (20 deg). No code computes the V flank chip of a plunge, so no rule sees that every V plunge is under the 0.025 mm floor. | high |
| (b1) op 16 dpp 2.64 (operator 2.0) | The calculator starts the adaptive depth at `adaptive_doc_factor` x tip D = 4.0 mm. The dial scales that default to 2.643 and writes it over the operator's 2.0. | high |
| (b2) op 16 fz 0.084 | A tapered query falls back to the straight 1/16 in solid-carbide ball-nose 1 x D row (log-log x1.10). The 15 x D stick-out does not lower the feed (R4). | medium-high |
| (b3) op 17 fz 0.114, feed 4105 | The Amana ZrN v8 1/16 in tapered cell is self-inconsistent: its printed IPM gives 0.039-0.064 mm, its printed chip 0.076-0.127 mm. The row keeps the printed chip; Suggest ships the midpoint x1.12. Review C's band equals the IPM-derived value. | high |
| (b4) short moves / acceleration | Pre-sim Suggest does not read the machine kinematics. Only the post-sim gate reads the achieved feed. | high |
| (b5) plunge 240 (ops 16, 17) | The G10 tapered claim covers tips 0.5-1.5875 mm only. A 2 mm tip falls to the material base 240 mm/min. | high |
| (c) op 15 0.94 / 4.70 / 18000 / fz 0.131 | The calculator starts at 2 x D x 0.2 x D (rigidity). The vendor envelope cuts ap to 1 x D. The dial then scales both ap and ae by 0.78 (0.85 x 0.88 L/D). The feed stays at the Spektra 1 x D point 0.131 (R4: no L/D on the feed). The rpm is the chart rpm. | high |
| (d) shared causes | (1) `FeedsHints::NONE` + family default ap/ae; (2) ruling R4: the long-tool share only moves geometry, never the feed; (3) the band midpoint / printed point as the target; (4) `match_chart` rpm; (5) G1 size claim. | high |

## (a) The V-bits

### (a1) Op 9: fz 0.127 mm/tooth at 18000 rpm (reviewers 0.035-0.057)

**Symptom.** Suggest keeps 2286 mm/min at 18000 rpm, 1 flute: fz 0.127 mm. Review B ships
0.035 / 0.045 / 0.057 mm. Both sides use the whole-edge chip (B method 1 = rs_cam ruling B4).
Thus the diameter is not the cause.

**Code path.**

1. `core/compute/catalog/registry.rs:1364`: Project Curve declares `feeds_hints: FeedsHints::NONE`.
   The calculator gets no axial depth (`core/feeds/suggest.rs:995`, `core/feeds/suggest/apply.rs:1196`).
2. `core/feeds/vendor_normalize.rs:118`: a V-bit on Project Curve routes to (Trace, Finish).
3. `core/feeds/mod.rs:3054-3057`: the (Trace, Finish) default is `ap_factor 0.06`, `ae_factor 0.02`.
   `ap = 0.06 x 3.175 = 0.1905 mm` (record 9: `axial_depth_mm 0.1905`).
4. Row `amana-vgroove-hardwood-trace-18deg-1f`
   (`crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json:84-107`):
   18000 rpm, band 0.0762-0.1778, `ap_rule` "2xD reduce chip load 25%".
5. `core/feeds/vendor_lookup.rs:1082` `chipload_midpoint`: (0.0762 + 0.1778) / 2 = 0.127.
6. `core/feeds/mod.rs:2224-2233`: `depth_tier_multiplier(ap = 0.19, d = 3.175)` = 1.0, so
   `raw_feed = 18000 x 0.127 x 1 = 2286`.
7. `core/feeds/mod.rs:2236-2254`: L/D 7.9 gives `LongToolDerate factor 0.75`. "The feed does NOT
   take it." `core/feeds/suggest/aggressiveness.rs:173`: a Finish pass returns
   `AggressivenessNotApplied`, so the share has no effect at all on op 9.

**Root cause.** Suggest takes the band midpoint at a depth of 0.06 x D that the op does not cut.
The real depth 6.42 mm is 2.02 x D, where the vendor's own rule removes 25 %. The long-tool share
(0.75) has no lever on a finish op with no depth or stepover field.

**Evidence.** 0.127 = midpoint (asserted by arithmetic). The vendor low end x 0.75 = 0.057, the top
of review B's band. Review B also lowers the chip for the 0.3 mm tip and the 25 mm stick-out
(judgement). The operator value is the same 2286, so the class is S = O.

**Fix options.**

- F1. Give Project Curve (and Trace) an axial hint: the depth of the curve below the stock top
  (Project Curve) or the op's `depth` (Trace). Then the B4 de-rate at the nominal D applies
  (x0.75 at 2 x D). Blast radius: `operation_feeds_hints` and its pin test
  (`core/feeds/suggest/tests.rs:2544`), the FM1 golden cells for Trace / Project Curve, and
  `axial_envelope.rs:184` (it reads the same hint). Op 5 (dangerous, root cause 02) has the same
  missing hint.
- F2. Seed a band with the low end (or a named fraction) instead of the midpoint on a finish trace.
  Blast radius: every banded row; the FM1 matrix moves widely. Needs an operator ruling (the
  midpoint-vs-maximum question was deliberately left open on 2026-08-19, `core/feeds/mod.rs:2190-2200`).
- F3. Let the long-tool share act on the feed where no geometry lever exists (finish ops). This
  reverses part of ruling R4 Q7; see (d).

Sentries: `a_vbit_row_is_read_at_its_printed_key_b4`, `long_tool_derate_is_shown_ld1`,
`the_dial_holds_the_load_and_never_cuts_the_feed_fm7`, `one_depth_derate_for_feed_and_band_fm3`,
`feeds_matrix_instrument_fm1` (re-bless), unit test `operation_feeds_hints_is_an_explicit_per_op_decision`.

Confidence: high.

### (a2) The flat tip of the V20 ("rs_cam models a sharp tip")

**Code path.**

1. `core/tool/vbit.rs:33-38`: `VBitEndmill::tip_diameter` exists. `new()` sets 0.0 (`:62`).
   `geometry_hint` and `flat_tip_diameter` pass it on (`:116-125`).
2. `core/compute/tool_config.rs:444-480`: `ToolConfig` has no tip field. Thus the project TOML
   cannot carry the 0.3 mm.
3. `core/compute/tool_config.rs:589-592`: `geometry_hint()` writes `tip_diameter: 0.0`
   ("`build_cutter` builds every V-bit pointed").
4. `core/compute/cutter.rs:32-37`: `build_cutter` never sets `tip_diameter`.
5. Feeds readers of the tip: `core/feeds/mod.rs:186-191` (`engaged_diameter_at_doc`) ignores the tip
   (`..`); `core/feeds/mod.rs:3522-3528` (`effective_diameter`) reads it, but only for the chip-thinning
   factor, which is computed and not applied (`core/feeds/mod.rs:2212`).

**Root cause.** The tip value has no door from the tool record to the cutter. For the feeds
numbers this has no effect: B4 keys the row at the angle (no diameter), the row prints the rpm
(18000 overrides the engaged-D SFM at `core/feeds/mod.rs:1944`), and chip thinning is not applied.
The geometry effect is large: a 0.3 mm flat on a 20 deg V moves the cone apex by
0.15 / tan 10 deg = 0.85 mm. At one Z the real groove is 0.3 mm wider than the model; for one width
the real tip sits 0.85 mm higher.

**Fix options.** Add `tip_diameter` to `ToolConfig` (default 0, serde default), pass it in
`build_cutter` and `geometry_hint`. Blast radius: the TOML schema, the GUI tool editor, `domain.rs`
bounds, every V-bit drop-cutter / v-carve / inlay consumer (`ops/inlay.rs`,
`compute/execute/curve_engrave.rs`, `maps/tool_shape_key.rs` already read `flat_tip_diameter`).
Feeds output does not move. Sentries: `geometry_hint_matches_the_built_cutter`
(`core/compute/tool_config.rs:1696`), `engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`
(`core/feeds/tests.rs:1848`), `tool_geometry_hygiene`, `a_tool_outside_its_domain_is_refused_at_every_door_f029`.

Confidence: high (no feeds effect; the geometry offset is derived).

### (a3) Op 10 depth per pass 0.371 (operator 2.598) and op 13 0.3 (operator 0.6)

**Symptom.** Op 10 cuts 5.196 mm; Suggest writes 0.3711 = 5.196 / 14. Op 13 cuts 0.6 mm; Suggest
writes 0.3 = 0.6 / 2.

**Code path.**

1. `core/compute/catalog/registry.rs:522`: Trace declares `feeds_hints: FeedsHints::NONE`;
   `:507-508` (Trace, Finish).
2. `core/feeds/mod.rs:2075-2076` and `:3054-3057`: ap = 0.06 x 6.35 = 0.381 mm (record 10:
   `axial_depth_mm 0.381`). The operator's depth per pass is not an input.
3. `core/feeds/suggest/apply.rs:160`: `depth_mm = round(result.axial_depth_mm)`.
4. `core/feeds/suggest/apply.rs:179-181`: `realised_step_down(total, 0.381)`
   (`core/ops/depth.rs:150`): ceil(5.196 / 0.381) = 14 passes, 0.3711 mm; ceil(0.6 / 0.381) = 2, 0.3 mm.
5. `core/feeds/suggest/aggressiveness.rs:173`: Finish, so no dial; `registry.rs:527`
   `axial_envelope: NotApplied`. Nothing else moves the depth.

**Root cause.** Suggest writes the family-default finish depth (0.06 x D, an unsourced repo value)
into a Trace op whose depth is the design of the cut. The value has no basis for a V-bit, whose
load scales with the groove area, not with D. Ruling R2 ("most flexibility for the user",
`tests/finish_depth_is_reported_not_capped_fm6.rs:1-10`) says a finish depth is not capped; here
the operator's finish depth is replaced, which is a stronger action than a cap.

**Evidence.** 14 x 0.3711 = 5.196 and 2 x 0.3 = 0.6 (arithmetic). Rough-score generated 2 passes at
the operator value and at 3.67 (`compare.md`, gate table).

**Fix options.**

- F1. Give Trace an axial hint (the op's `depth_per_pass`, or the full `depth` for one pass). Then
  the calculator sizes the chip at the real ap and apply writes back the same value.
- F2. On a Finish pass, do not write the calculator depth into an op whose depth the operator
  set (keep the depth; report the calculator value as a rationale entry, as Project Curve already
  does with `cut_geometry_field_not_held`).
- F3. For a V-bit, size the pass by the groove area (equal area per pass), not by D.
Blast radius: Trace and VCarve FM1 cells; `apply.rs` depth snap is shared by every 2.5D op, so F2
must test the pass role and the tool class, not change the snap. Sentries:
`finish_depth_is_reported_not_capped_fm6`, `a_speeds_only_apply_ships_the_feed_for_the_depth_it_keeps_g_speedsapply`,
`feeds_matrix_instrument_fm1`, the pin test `operation_feeds_hints_is_an_explicit_per_op_decision`.

Confidence: high.

### (a4) Op 10 and op 13 rpm 18000, feed 2743

Row `amana-vgroove-hardwood-trace-60deg-2f` (`amana_vgroove_engraving.json:240-264`) prints
18000 rpm and one point 0.0762 mm (a 1/2 in carbide-tipped Zero-Point bit). `core/feeds/mod.rs:1944`
takes the chart rpm (`match_chart`). Feed = 18000 x 0.0762 x 2 = 2743. Review B agrees for op 10.
For op 13 review B wants 0.04-0.06 (a 0.6 mm engraving, Amana engraving chart S3); `compare.md`
marks it high but safe. The row is a 1/2 in tipped bit applied to a 1/4 in solid V (B4: keyed at the
angle). No risky item here; the operator's 10026 rpm is the cause of the operator's low class.

### (a5) The V plunge: 381 mm/min (op 9) and 914 mm/min (ops 10, 13)

**Code path.**

1. `core/feeds/plunge.rs:258-298` `plunge_basis`: a V-bit keys the claim by its angle.
2. `core/feeds/extrapolation/plunge.rs:313-320`: `g10_plunge_vbit60` = 1/3 x feed at 60 deg +-0.5,
   2 flutes. Op 10/13: 2743 / 3 = 914.
3. Op 9 (20 deg) has no claim: `MaterialBase` = `core/material/mod.rs:1369` `plunge_rate_base`:
   720 x clamp(3.175 / 6, 0.25, 3) = 381.
4. `core/tool_load/plunge_stress.rs:42-47`: a V-bit has no tip cap (`None`).
5. The rubbing floor (`core/feeds/mod.rs:1184`, used at `:2807`) tests the side chip only.

**Root cause.** No code computes the chip of a V plunge. A V edge in a plunge takes
`axial feed per tooth x sin(half angle)`: op 9 0.0212 x sin 10 deg = 0.0037 mm; ops 10/13
0.0254 x sin 30 deg = 0.0127 mm. Both are under the 0.025 mm floor. To reach the floor the plunge
must be 2591 mm/min (V20, 1F) or 1800 mm/min (V60, 2F) at 18000 rpm. The V20 value is over the
Z rate of the machine, so a faster plunge is not a fix there.

**Fix options.** Add a warn-only plunge-chip check for a V-bit (flank chip vs the R4 floor), and
state "fewer plunges" as the action (Trace forces no entry: `registry.rs:514` `FORCE_NO_ENTRY`;
Project Curve strips every dressup). Blast radius: `plunge.rs` and the explain card; no shipped
number moves if the check only warns. Sentries: `a_plunge_is_a_named_fraction_of_the_side_feed_g10`,
`rubbing_floor_warns_and_never_lifts`, `suggest_leaves_the_entry_style_to_the_operator_g10`.

Confidence: high (arithmetic; reviewer B derived the same 0.144 mm/rev floor).

## (b) The R1 x 6 x 20 tapered ball (ops 16, 17)

Tool record: `diameter 2.0` (tip), `taper_half_angle 5.7106`, `shaft_diameter 6.0`, `stickout 30`,
2F. `geometry_hint` gives `TaperedBall { tip_radius 1.0, taper 5.71 }`
(`core/compute/tool_config.rs:593-596`).

### (b0) The effective-diameter model

`core/feeds/mod.rs:166-181` (`engaged_diameter_at_doc`) calls `tool::ball_cone_radius_at_height`:
ball below the tangency height, cone above, capped at the shank. This matches review C's
D(h) = 2 (R cos a + (h - h_t) tan a) (D(2.0) = 2.21 mm); `compare.md` checked C's De values. The
model is not a cause. Its uses: the depth ladder (`core/feeds/mod.rs:2224-2232`, R2) and the band
de-rate. The LUT key is the tip (ruling A1). Chip thinning (radial and axial) is computed and not
applied (`core/feeds/mod.rs:2212`, ruling 2026-08-19). Thus Suggest never ships the h = fz x De/D
or the RCTF lift that review C uses.

### (b1) Op 16 depth per pass 2.643 (operator 2.0)

**Code path.**

1. Adaptive3d reads (Adaptive, Roughing): `core/feeds/mod.rs:2981-2984` `ap_factor 1.50`; the
   tapered base `ADAPTIVE_TAPERED_AP 0.70` (`:3244`).
2. `core/feeds/mod.rs:3433-3436`: `ap_factor = max(0.70 x hard scale, adaptive_doc_factor 2.0 x
   1.50 / 1.50)` = 2.0. `ap = 2.0 x d` with `d` = the tip, 2.0 mm, so ap = 4.0 (record 16).
3. `core/feeds/suggest/aggressiveness.rs:146-161`: target share 0.85 x 0.75 (L/D 15) = 0.6375.
   Depth and stepover both scale to 66 %: 4.0 -> 2.643. The op has a `depth_per_pass` field, so
   the value is written over the operator's 2.0.

**Root cause.** The dial scales the calculator's default depth, not the operator's depth. The
default is the rigidity floor `adaptive_doc_factor x D` taken at the tip diameter of a tapered
ball. A tapered ball at 2 x tip D engages De about 2.3 mm, so the rule gives a deeper cut than the
same rule gives a straight 2 mm tool. The dial reads the result as "less load" only relative to
4.0; the shipped 2.643 is 32 % deeper than the operator's planned load.

**Fix options.** (1) Seed pass 6b from the op's own depth when the operator set one (the dial then
only lowers). (2) Take the tapered default at the engaged D or cap it at the tip D for a rest op.
Blast radius: every Adaptive / Adaptive3d cell (FM1 re-bless), `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`,
`suggest_feed_matches_final_geometry`. Confidence: high.

### (b2) Op 16 fz 0.084 mm (reviewers 0.033-0.055)

**Code path.**

1. `core/feeds/vendor_lookup.rs:1054-1063, 1074`: a `TaperedBallNose` query is compatible with
   `BallNose` rows (score 110 vs 220 for a tapered row).
2. The chosen row is `amana-ball-hardwood-pocket-1588-2f-v7`
   (`data/vendor_lut/observations/amana_ball_nose.json:930-945`): a straight solid-carbide
   1/16 in ball nose, 1 x D chart, 0.0508-0.1016 at 18000 (the printed IPM agrees: 70-150 IPM =
   0.049-0.106 mm).
3. `core/feeds/extrapolation/size.rs:83, 180` (G1 form A): log-log between the 1.5875 and 3.175
   rows, x1.10 at the 2 mm tip.
4. The 15 x D stick-out does not reach the feed (`core/feeds/mod.rs:2236-2240`).
5. `core/feeds/suggest/invariants.rs` pass 9 (`final_geometry_rescale`): depth tier 0.8669 at
   ap 4.0 -> 0.9675 at ap 2.643: feed 2702 -> 3016.

**Root cause.** The chip is a short, straight ball-nose chip at 1 x D, interpolated up to 2 mm,
with no factor for a 15 x D projection. Review C starts from tapered carving charts (Amana 3F,
Onsrud 77-100) and applies x0.70 for the projection. The lookup score chose the ball row over the
Onsrud 77-100 tapered home row that `FAMILY_RULES` serves to adaptive (`core/feeds/extrapolation/family.rs:94-116`);
the exact score difference was not traced (medium).

**Fix options.** F3 of (d) (L/D on the feed), or a rule that a tapered query prefers a tapered row
inside its size range before the ball fallback. Sentries: `a_tapered_row_is_read_at_the_tip_a1`,
`lookup_parity`, `lut_resolver_census_a6`, `a_generic_tool_reads_the_standard_row_first_g_series`.
Confidence: medium-high.

### (b3) Op 17 fz 0.114, feed 4105 at 18000 (reviewers 0.040-0.052)

**Code path.**

1. Scallop routes the tapered ball to the ZrN v8 tapered rows (G3, operator ruling 2026-10-01):
   `amana-tapered-hardwood-parallel-1587-2f-zrn-v8`
   (`data/vendor_lut/observations/amana_zrn_tapered_v8.json:149-163`), printed 0.0762-0.127.
2. G1 form A x1.12 toward the 6.35 mm row: band 0.0853-0.1422 (record 17 bounds).
3. `core/feeds/vendor_lookup.rs:1082`: midpoint 0.1138. Feed = 18000 x 0.1138 x 2 = 4105.
4. Scallop is a Finish pass: no dial (`aggressiveness.rs:173`), so the L/D share does nothing.

**Root cause.** The row notes state: "CHART SELF-INCONSISTENT: the printed IPM 55-90 at 18,000 RPM
x 2 flutes gives 0.0015-0.0025 in/tooth (derived), half the printed chip load". The row keeps the
printed chip (grade b). 55-90 IPM / 36000 = 0.039-0.064 mm. Review C's 0.040-0.052 lies inside the
IPM-derived band. Suggest ships the midpoint of the printed band, x1.12, which is about 2 x the
chart's own feed.

**Fix options.** (1) Hold the self-inconsistent cell at the IPM-derived band (or the overlap of the
two), as the deleted pre-2026-09-24 row did. (2) Refuse the cell (support `Refuse`) until a
ruling. Blast radius: every tapered finish cell that reads this row (`the_printed_tip_row_serves_every_3d_finish_g3v8`,
`the_micro_tapered_finish_ships_the_printed_tip_row_g1`, FM1 re-bless). This is a data ruling, not a
code bug. Confidence: high.

### (b4) Short moves and acceleration

`rg accel core/feeds/` finds only the post-sim explanation: `core/feeds/feed_explanation.rs:221-245`
(stage 3, achieved / commanded feed) and `core/tool_load/mod.rs:72` (`effective_feed_for_sample`).
Pre-sim Suggest commands 4105 mm/min and assumes the path holds it. The machine has
`acceleration_mm_s2 = 983` (project), and review C finds a real feed of 1000-3500 mm/min (Z rate
and curves), so the real chip at 18000 rpm varies 0.028-0.097 mm. The feed modulation and
kinematic-utilisation code (`core/machine/kinematic_utilization.rs`, `core/dressup/feed_modulation.rs`)
needs a generated path, and Suggest runs before generation.

Fix option: a post-generation pass that reads the commanded-vs-achieved distribution
(the stage 3 ratio) and reports the chip at the median achieved feed. Blast radius: Suggest would
need the toolpath (a new dependency on generation); keep it in the post-sim gate. Confidence: high.

### (b5) Plunge 240 (ops 16, 17)

`core/feeds/extrapolation/plunge.rs:296-310`: `g10_plunge_tapered` covers tips 0.5-1.5875 mm. A
2.0 mm tip is outside, so `MaterialBase` = 720 x clamp(2 / 6) = 240; the tip cap 150 x 2 = 300 does
not bind (`core/tool_load/plunge_stress.rs:45-47`). 240 / 36000 = 0.0067 mm/tooth axial (rubs).
The entries are 3 deg ramps at the cut feed, so the rate is rarely used. Fix: widen the tapered
claim only with a printed 2 mm witness (no bench rig). Sentry: `a_plunge_is_a_named_fraction_of_the_side_feed_g10`.

## (c) Op 15, 6 mm flat front rough: stepover 0.94, dpp 4.70, rpm 18000, fz 0.131

**Code path.**

1. `core/feeds/mod.rs:2981-2984, 3433-3450`: Adaptive roughing: ap = 2.0 x 6 = 12.0; wood + flat
   gives ae = `adaptive_woc_factor` 0.2 x 6 = 1.2.
2. `core/feeds/suggest/axial_envelope.rs` (record: "clamped 12.00 -> 6.00 mm (vendor_ap)"): the
   row's `ap_max_factor 1.0` cuts ap to 1 x D.
3. `core/feeds/suggest/aggressiveness.rs:160-161`: share 0.85 x 0.88 (L/D 5) = 0.748; both levers
   scale to 78 %: 6.0 -> 4.702, 1.2 -> 0.940.
4. Row `amana-flat-hardwood-pocket-6000-2f-spektra`: one printed point 0.131 (A2) at 18000 rpm
   (`core/feeds/mod.rs:1944`, `match_chart`).
5. Pass 9: the calculator feed 3540 had depth tier 0.75 (ap 12 = 2 x D); at 4.70 the tier is 1.0,
   so the feed rises to 4721.

**Root cause.** The dial holds the load by shrinking both depth and stepover, even after the vendor
envelope already cut the depth by half; the L/D share (0.88) acts on the stepover, not on the
chip. The chip stays at the Spektra 1 x D point (a coated, short-tool chart value). Review A wants
the opposite trade: wider stepover (1.38-1.8) and a lower chip (hex 0.051-0.080) at 30 mm
stick-out. The rpm 18000 is the chart rpm; the operator's 20102 is inside review A's band.

**Fix options.** F3 of (d); or apply the dial only to the lever that the envelope did not already
bind. Sentries: `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`, `long_tool_derate_is_shown_ld1`,
`suggest_feed_matches_final_geometry`, `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
`feeds_matrix_instrument_fm1`. Confidence: high.

The plunge 2360 (feed / 2, `g10_plunge_flat`) is risky only because centre cutting is not known:
no code under `core/feeds/` reads `non_centre_cutting` (`rg` finds only `predict.rs:875`). This is
the same gap as the dangerous items of ops 6-8.

## (d) Causes shared with the dangerous items

1. **No axial or radial hint for most ops: the calculator sizes a cut the op does not make.**
   `core/compute/catalog/registry.rs:522` (Trace) and `:1364` (Project Curve) `FeedsHints::NONE`;
   `core/feeds/mod.rs:2076` `operation_default_profile`. Here: ops 9, 10, 13. Dangerous: op 5
   (root cause 02, no slot width, `NoRadialEngagement`).
2. **Ruling R4 Q7: the long-tool share never lowers the feed.** `core/feeds/mod.rs:2236-2240`
   ("The feed does NOT take it") and `core/feeds/suggest/aggressiveness.rs:173` (no dial on a finish).
   On a finish op the share is lost; on a rough it shrinks the stepover (time) and keeps the
   short-tool chip. Here: ops 9, 15, 16, 17. Dangerous: op 2 (fz 0.131 at 5 x D on the 14.5 mm entry)
   and op 5.
3. **The band midpoint or the printed point is the target, with no fraction for an unrated set-up.**
   `core/feeds/vendor_lookup.rs:1082`. Here: ops 9, 17. Dangerous: op 5 (Onsrud band 0.356-0.406).
4. **The spindle strategy `match_chart`** takes the chart rpm (`core/feeds/mod.rs:1944`); Suggest
   never moves to 20000-24000 rpm to thin the chip. Here: ops 15, 16 (review C: 24000). Dangerous:
   ops 0, 1, 14 (RpmLoweredForFeedCeiling).
5. **The G1 size claim** (`core/feeds/extrapolation/size.rs:83-87`): form A (log-log between printed
   rows) gives ops 16 (x1.10) and 17 (x1.12); form C (d^0.61) gives the dangerous ops 0, 1, 14 (x1.24).
   Form A here is mild; the size claim is not the main cause for the small tools.
6. **The dial seeds from the calculator default, not from the operator's value**
   (`core/feeds/suggest/aggressiveness.rs:146`): op 16 depth 2.643 here; ops 1 and 14 depth 14.047
   (dangerous) come from the same seed.

## Not checked

- The exact lookup score that put the Amana ball row above the Onsrud 77-100 tapered row for op 16.
- No rough-score run: ops 15-17 are rest / finish ops whose generation needs the ancestor
  simulation (`compare.md`); the op 10 pass count is already in `compare.md`'s gate table.
