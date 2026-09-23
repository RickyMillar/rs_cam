# R4 specification: the aggressiveness dial replaces the hidden margins

Date: 2026-09-23 (evening). Status: SPEC. No number moves before the operator
reads this file. Owner of the ruling: the operator. Evidence rows are in
`EVIDENCE.md`. The chain review is `DERATE_CHAIN_REVIEW.md`. The counts are
from `matrix_2026-09-23.csv` of tonight (FM1 on the R1 evidence commits,
498 of 960 cells ship).

## 0. The ruling and the decision already taken

The operator ruled R4 in the second round (`RULINGS.md`, "Operator rulings,
second round"):

> the hidden 25 % safety factor goes; the only margin is the machine
> aggressiveness dial, and it must keep the chipload in the vendor band and
> reduce load through engagement (depth and width), not shave feed or depth
> alone ("more finessed than that. keeping chiploads but reducing load"): a
> spec before any number moves. The long-tool de-rate may stay as a minor
> cut but must show in the UI when it fires. The floor: [...] keep the
> warning, drop the clamp.

The orchestrator took one decision before this spec:

- **The 0.75 factor does not move tonight.** It leaves only in the same
  change that ships the dial (WP3). A margin that leaves before its
  replacement arrives raises the load on 453 shipping cells with no margin
  at all.

Tonight two packages are safe: WP1 (the long-tool de-rate becomes visible)
and WP2 (the rubbing floor warns and does not lift). WP3 (the dial and the
0.75 removal) waits for the operator's look at sections 2 and 5.

## 1. The chain today

### 1.1 Where the code contradicts `DERATE_CHAIN_REVIEW.md`

The code wins in each case.

1. **The floor fires on 80 shipping cells, not 333.** The review counted
   before R5 (827f383e..d884ab7a) and before the R1 evidence refusals
   (94b80b24, 19e98315). Tonight 80 of 498 shipping cells fire
   `ChiploadClampedToFloor`.
2. **One depth ladder, linear between the printed points.** R3 (aef54c83)
   joined S4 and S7 into `geometry::doc_derating_scale`
   (`feeds/geometry.rs:155`). At 1.5 x D it gives 0.875, not the step value
   0.75 that the review gives for cell 6. The shown band is de-rated at the
   shipped depth (`feeds/mod.rs:2431-2463`), not at the depth hint.
3. **The 0.45 tier is gone.** Verified in code: `doc_derating_scale` holds
   0.50 above 3 x D (`geometry.rs:163-164`), and
   `depth_beyond_published_table` (`geometry.rs:175`) raises the Caution
   `feeds.depth_beyond_published_table`
   (`diagnostics/adapters/from_static_checks.rs:372-376`).
4. **The eight cell values in the review predate R5.** Tonight cell 1
   (EndMill d6 Pocket softwood) ships 3000 mm/min at the machine ceiling
   (row chip 0.2032, band 0.1778-0.2286). Cell 6 (EndMill d6 Adaptive
   softwood) ships 2640 mm/min: 0.127 x 0.875 x 0.88 x 0.75 x 36000.
5. **The line numbers in the review are stale.** Section 1.2 gives the
   current lines.
6. **The margin is not hidden, but its label is wrong.** The machine panel
   already shows `safety_factor` as a slider called "Aggressiveness:"
   (`crates/rs_cam_viz/src/ui/properties/machine_panel.rs:199-224`, range
   0.60-0.95). The label does not say that the slider multiplies every feed.
   The Shapeoko presets carry 0.80, not 0.75 (`machine/mod.rs:322`, `:344`).

### 1.2 Every stage between the vendor row and the shipped recipe

"Fires" is the count of shipping cells where the stage is not 1.0. "Moves"
is the count where the shipped feed changes if the stage alone becomes 1.0.
The two differ because the floor and the machine ceiling mask other stages.
I reconstructed each cell as `r_chip_load_mm x tier x L/D x 0.75`, then the
ceiling, then the floor. The model predicts the floor on the same 80 cells
that fire it. Of the 323 cells that neither the floor nor the ceiling
touches (a different set of 323 from the banded cells in 1.3), 305
reproduce to 0.3 %. The other 18 are Adaptive3d cells where
Suggest pass 9 re-derives the feed at the final depth (S16). They are not a
defect.

| # | Stage | Code (current lines) | Source status | Fires | Moves |
|---|---|---|---|---|---|
| S0 | Vendor row midpoint, or the formula `k0 D^p (1/H)^q` | `feeds/mod.rs:1395-1445`; `vendor_lookup.rs` | row: published where R5 printed it; formula: `FormulaOnly`, BACKED only (R1) | 498 | n/a |
| S1 | Drill multiplier 2.5 | `feeds/mod.rs:1440-1445` | unsourced (6-8) | 0 (every drill cell refuses, R1) | 0 |
| S2, S3 | Diameter `^0.61` and hardness `^0.5` transfer | `vendor_lookup.rs` | partial | not counted | not counted |
| S3b | Default engagement `ap_factor`, `ae_factor` per family and role | `feeds/mod.rs:2647-2894` (`operation_default_profile`, `default_engagement`) | unsourced (R2 territory) | 498 | n/a |
| S5 | RPM (row nominal, or 200 m/min) | `feeds/mod.rs:1360-1740` | row value or repo value | not counted | not counted |
| S6 | Chip thinning | `feeds/mod.rs:1857-1864` | observed, not applied | 0 | 0 |
| S7 | Depth ladder `depth_tier_multiplier` | `feeds/mod.rs:1891`; `geometry.rs:155`, `:287` | **published** (Onsrud, Freud, Amana) | 64 | 64 |
| S8 | L/D de-rate 0.88 (> 4) / 0.75 (> 6) | `feeds/mod.rs:1895-1912` | unsourced (5.2-14) | 470 (0.88 on 249, 0.75 on 221) | 317 |
| S9 | Workholding 0.85 / 1.00 / 1.03 | `feeds/mod.rs:1913-1922` | unsourced | 0 (instrument runs Medium) | 0 |
| S10 | Power ladder: RPM, then depth, then width, then feed | `feeds/mod.rs:2011-2318` | machine fact; budget = `power_at_rpm x safety_factor` | 0 | 0 |
| S11 | Machine cutting-feed ceiling (4000 on the generic router) | `feeds/mod.rs:2321-2335` | machine fact | 95 | 95 |
| S12 | Safety factor 0.75 on feed, plunge and ramp | `feeds/mod.rs:2347-2350`; `machine/mod.rs:293` | unsourced (5.2-14) | 498 | 453 |
| S13 | Rubbing-floor lift `effective_rubbing_floor` | `feeds/mod.rs:1056`, `:1100`, `:2464-2497` | unsourced (4.1-1 to 4.1-4); contradicted (4.1-26) | 80 | 80 |
| S14 | Drill envelope, RPM follows down | `feeds/mod.rs:2499-2566` | unsourced (4.3-14) | 0 | 0 |
| S15 | Round down to 1 mm/min | `feeds/suggest/apply.rs` | engine rule | all | < 0.2 % |
| P4 | Suggest roughing depth clamp `doc_roughing_factor x D` | `suggest/invariants.rs:353-391`; `machine/mod.rs:114-141` | unsourced rule of thumb (5.1-12) | 168 (`RoughingDepthClampedToRigidity`) | depth, not feed |
| P6 | Suggest deflection back-off, 200 um | `suggest/invariants.rs:431-535` | engine rule; roughing only | 0 (`DppCappedByDeflection`); 20 `DeflectionBackoffFigureIsAFloor` | 0 |
| S16 | Suggest pass 9 re-derive at the final depth, and its floor lift | `suggest/adaptive_entry.rs:343-495` | engine rule | 24 (`FeedRescaledToFinalGeometry`) | 24 |

The floor detail (tonight): 63 of the 80 floor cells have a band, 17 have
none. 10 take the arm that parks the recipe on the band ceiling
(`RubbingFloorCappedToBandCeiling`). 5 are lifted to the band maximum. The
floor cells are BallNose d3.175 (37), BallNose d6 (24), EndMill d3.175 (16)
and BullNose d3.175 (3).

### 1.3 What the chain does to the band, tonight

323 of the 498 shipping cells carry a band.

| Case | In the band | Below the band minimum | Above the band maximum |
|---|---|---|---|
| Today (0.75, L/D, floor, ceiling) | 55 | 268 | 0 |
| Today without the floor lift | 0 | 323 | 0 |
| Safety factor 1.0, L/D kept, no floor | 183 | 140 | 0 |
| Safety factor 1.0, L/D 1.0, no floor | 246 | 77 | 0 |

The other 175 shipping cells carry no band (for example the Amana Spektra
rows that print one value per size, the R5 follow-up). There the rule "keep
the chip in the band" means: ship the printed value times the depth ladder.

All 77 cells in the last row are cells where the machine ceiling S11 binds.
The ceiling cuts the feed at the same RPM, so the chip falls under the band.
The operator's rule "keep the chipload in the band" therefore meets three
obstacles: the 0.75 factor, the L/D factor, and the ceiling. Section 2
removes the first. Section 3 names the other two.

### 1.4 The 0.75 factor has two jobs

`MachineProfile::safety_factor` (`machine/mod.rs:185`) does two different
things:

1. **A feed scale.** Step 9 multiplies the feed, the plunge and the ramp
   (`feeds/mod.rs:2347-2350`). `commanded_cutting_feed_ceiling_mm_min`
   (`machine/mod.rs:250-252`) multiplies the ceiling. This is the "hidden
   25 %" of the ruling.
2. **A power budget fraction.** Each power ceiling is
   `power_at_rpm x safety_factor`: `feeds/mod.rs:1997` (Step 6 gate power),
   `:2033` and `:2104-2105` (the ladder budget),
   `suggest/adaptive_entry.rs:545-599` (pass 10),
   `feeds/operating_point.rs:240`, `tool_load/power.rs:481`, `:578`,
   `session/compute.rs:1861`, `tool_load/optimize/mod.rs:701`,
   `tool_load/optimize/strategy/headroom.rs:84`, `:476`,
   `tool_load/verdict.rs:819`, `:885` (`BoundSource::MachinePowerCurve`)
   and `feeds/explain_payload.rs:32`, `:48`.

WP3 must give each job its own fate. Section 3.1 does that.

## 2. The dial

### 2.1 Name, place, range, default

- **Name:** `MachineProfile::aggressiveness: f64`. It replaces the field
  `safety_factor`. The GUI label stays "Aggressiveness".
- **Place:** the `MachineProfile`, not a project setting. Recommended,
  because the quantity is a statement about the machine (its stiffness and
  the operator's trust in it), the panel row already exists
  (`machine_panel.rs:199-224`), and the machine rides in the project file
  already.
- **Range:** 0.50 to 1.00, step 0.05. Values above 1.00 are refused (Q3).
- **Default:** 0.75 on every preset (Q1). Section 2.4 gives the reason.
- **Serde:** no alias for `safety_factor` (operator ruling 2026-09-16, "no
  legacy, breaking OK"). The commit states the break. A project file that
  carries `safety_factor` loads with the default dial.

### 2.2 What the dial is, and what it is not

The dial sets the **load** of the cut as a fraction of the load at the
**base engagement**. The chipload does not move.

- The **chipload** is the vendor band midpoint times the published depth
  ladder (S0 x S7). The dial never multiplies it.
- The **base engagement** is the depth per pass and the stepover that
  Suggest ships at dial 1.00: after the default engagement (S3b), the
  scallop stepover, the flute guard, the Suggest clamps P4 and P5 and the
  deflection back-off P6.
- The **load** is two predicted quantities at the held chipload:
  - the peak lateral force `F`, from `feeds::force::lateral_cutting_force`
    (`feeds/force.rs:141`). The tip deflection is linear in `F` for one
    tool, so `F` also stands for the deflection.
  - the spindle power `P`, from `feeds::power_model_terms`
    (`feeds/mod.rs:1251`) at the held feed.
  - For a material without a primary-source `Kc` (the force and power
    models refuse it), the proxy is the chip cross-section
    `ToolGeometryHint::mrr_cross_section_mm2(ap, ae)`. The warning names
    the proxy.

The dial is not:

- a chipload target inside the band (`SuggestAggressiveness`, which WP3
  deletes; see 3.7);
- a feed scale (the 0.75 factor, which WP3 deletes);
- the simulation feed-modulation scalar `modulation_aggressiveness`
  (`dressup/feed_modulation.rs:270`), which is a different quantity (Q6).

### 2.3 The algorithm

The dial runs as a new Suggest pass, "pass 6b", in
`feeds::suggest::invariants::enforce_invariants`, after
`backoff_dpp_for_deflection` (`suggest/invariants.rs:161-163`) and before the
entry-style pass. Reason: the base engagement is known only after passes 0
to 6, and pass 9 (feed re-derive at the final depth) and pass 10 (power
re-check) must see the dial's result. Put the pass in a new file
`crates/rs_cam_core/src/feeds/suggest/aggressiveness.rs`.

Steps:

1. Read `k = machine.aggressiveness`. If `k >= 1.0`, stop. Record nothing.
2. Read the base engagement `(ap0, ae0)` from the operation:
   `depth_per_pass()` and `stepover()`. Read the held chipload
   `fz = feed / (rpm x flutes)` from the calculator operating point in the
   `SuggestContext`.
3. Compute the base load `F0 = F(ap0, ae0)` and `P0 = P(ap0, ae0)` at `fz`.
   If both models refuse, compute `A0` (the proxy).
4. Define `fits(ap, ae)`: `F(ap, ae) <= k x F0` and `P(ap, ae) <= k x P0`
   (or `A <= k x A0` for the proxy). Both loads rise monotonically with
   `ap` and `ae`, so bisection is valid. Reuse `largest_fitting`
   (`feeds/mod.rs:1297`); make it `pub(crate)`.
5. Find the levers. A lever is a field that the operation has, that the
   apply scope writes, and that the engine chose:
   - the depth lever is `depth_per_pass`. It is a per-pass step, so the
     part does not change; the pass count rises.
   - the width lever is `stepover`, except where a scallop target set it
     (Step 3b, `feeds/mod.rs:1746-1766`).
6. Apply one common scale `s` to every lever that exists:
   `ap = s x ap0`, `ae = s x ae0`. Find the largest `s` in `(0, 1]` for
   which `fits` holds, by bisection. If only one lever exists, the scale
   acts on that lever alone. The floors are the calculator's own minimums
   `MIN_AP_MM = 0.05` and `MIN_AE_MM = 0.02` (`feeds/mod.rs`, Step 4). A
   lever that reaches its floor stops there, and the solve continues on the
   other lever. No new constant.
7. Write the new `ap`, `ae` to the operation. Do not touch the feed, the
   plunge or the RPM. Pass 9 then re-derives the feed at the final depth
   through the published ladder only, and pass 10 re-checks power.
8. File one `SuggestWarning::EngagementReducedForAggressiveness` with:
   `aggressiveness`, `dpp_from`, `dpp_to`, `stepover_from`, `stepover_to`,
   `force_n_before`, `force_n_after`, `power_kw_before`, `power_kw_after`
   (or the proxy pair), and `target_met: bool`.
9. If no lever exists, or a floor stops the solve, file the warning with
   `target_met: false` and the reason. **Never cut the feed to close the
   gap.**

Why one common scale, and not one lever first. The force model has an
edge intercept: `F = ap x (Ks x h + F_edge)`, with `h = fz x sin(theta)`
and `cos(theta) = 1 - ae / r` (`feeds/force.rs:19-26`). Below
`h = 0.106 mm` (most wood roughing) the edge term is the larger part. The
width reaches `F` only through `h`, so `F` responds to the width weaker
than to the square root of the width. Example: review cell 6 (EndMill d6
Adaptive, `ae0` 1.2 mm, chip 0.127): `Ks x h` = 5.0 N/mm and
`F_edge` = 5.3 N/mm. A width-only cut to `0.75 x F0` needs `ae` of about
0.22 mm, a 5.5 x smaller stepover, and the removal rate falls to about
18 %. A width-first order therefore either drives the width to its floor or
leaves the depth to do the work alone. The ruling forbids both. The depth
is linear in `F`, so the common scale gives the depth most of the force cut
and the width a share of the power cut.

Where the scale acts, by family and role:

| Family (`OperationFamily`) | Role | Levers | Note |
|---|---|---|---|
| `Adaptive`, `Pocket`, `Face`, `Contour`, `Parallel`, `Scallop`, `Trace` | Roughing, SemiFinish | depth per pass and stepover, one common scale | A stepover that a scallop target set is not a lever. |
| any | Finish | none (Q4) | The finish depth is the stock allowance; the finish width is the scallop target. R2 says deflection decides on a finish. |
| `Drill` | any | none | A drill cycle has no radial engagement. |
| `Trace` (VCarve, Chamfer, Inlay) | Roughing | depth per pass only, where the operation has one | The line geometry sets the width. |

At `k = 0.75` on a clearing pass, the common scale lands near `s = 0.8`.
The exact value depends on the share of the edge term in `F`.

### 2.4 The default, and why 0.75

The force model is affine in the chip: `F = ap x (Ks x h + F_edge)`
(`feeds/force.rs:19-26`). Compare a depth-only cut at the same removal rate:

- today: depth `ap`, chip `0.75 x fz`:
  `F_today = ap x (0.75 Ks h + F_edge)`.
- dial 0.75, depth lever: depth `0.75 x ap`, chip `fz`:
  `F_dial = ap x (0.75 Ks h + 0.75 F_edge)`.

`F_dial < F_today` because the edge term also falls. The shear power is the
same, and the edge power is lower. The proof does not depend on the lever:
any solve that meets `F <= 0.75 x F0` gives
`F = 0.75 x F0 < F_today = 0.75 x F0 + 0.25 x ap x F_edge`. So **at the default 0.75 the dial never
loads a cut more than today does**, the chip moves back into the band, and
the removal rate is the same for a depth-only cut. The common-scale cut removes
less per minute than a depth-only cut, because the width also falls. That is the cost of the ruling's "not
depth alone". The pass count rises by about `1 / s`.

### 2.5 The extremes

- **`k = 1.00`:** pass 6b does nothing. The recipe is the base engagement at
  the full chip. Compared with today, the feed rises by 1/0.75 = 1.33 on the
  453 cells that the 0.75 factor moves, and the load rises with it. This is
  "no margin".
- **`k = 0.50`:** the load halves. On small tools a floor can stop the
  solve (for example a d3.175 pocket at `dpp` 0.635 mm). Then the warning
  says `target_met: false` and names the floor. The feed stays.
- **No operating point:** when `SuggestContext::calculator_operating_point`
  is `None` (a hand-typed feed through `resolve_operation_invariants`),
  pass 6b stops at step 2, by the same rule as pass 9
  (`feeds/suggest.rs:195-203`). There is no derived chip to hold.
- **No lever** (Finish, Drill, a pinned field, or `ApplyScope::Speeds`,
  which does not write the cut geometry, `suggest/apply.rs:44-48`,
  `:179-180`): the dial does not act. Under `ApplyScope::Speeds` the pass
  computes the engagement that it would ship, does not write it, and files
  the warning with the geometry that it needs. The text says: "Apply the cut
  geometry to hold the load at N %."

### 2.6 What the operator sees

- **Machine panel** (`machine_panel.rs:199-224`): the slider writes
  `aggressiveness`, range 0.50-1.00. The line under it states the meaning
  in words: "Load at 75 % of full engagement. The chipload stays in the
  vendor band; the depth per pass and the stepover get smaller." The three
  labels (Conservative / Balanced / Aggressive) move to thresholds 0.65 and
  0.90. `panel_apply.rs:292` compares the new field.
- **Inspector card** (`ui/properties/feeds_speeds.rs:205-229` and
  `ui/feeds/compare.rs::draw_inspector_comparison`): the rationale rows for
  depth per pass and stepover carry a new `RationaleReason::Aggressiveness`
  entry (`feeds/rationale.rs:88`), for example "Depth per pass 4.0 -> 3.3
  mm, stepover 2.1 -> 1.7 mm: aggressiveness 0.75 holds the force at 75 %
  (38 -> 28 N). The chipload does not change."
- **Why sentence** (`ui/feeds/why.rs:262-280`): the row
  `("machine safety factor", d.safety_factor)` goes. The sentence names the
  dial as a load change, not as a feed factor.
- **Explore hover** (`ui/feeds/explore.rs:346-347`): the reason "machine
  safety margin" goes.
- **Compare** (`ui/feeds/compare.rs:792`): the field moves.
- **MCP:** `inspect_machine` (`rs_cam_viz/src/app/mcp/project.rs:537`)
  reports `aggressiveness`. `apply_feeds` returns the new warning in its
  reply. The explain payload (`feeds/explain_payload.rs:32`, `:48`) carries
  `aggressiveness`.
- **Diagnostics:** the warning maps to `feeds.aggressiveness_engagement`,
  Severity `Info` when `target_met`, `Caution` when not.

## 3. The fate of every unsourced scale

### 3.1 The safety factor 0.75 (S12): removed WITH the dial, not before

- The feed job goes: delete `feed *= machine.safety_factor`, the plunge and
  the ramp multiplies (`feeds/mod.rs:2347-2350`), the factor in
  `commanded_cutting_feed_ceiling_mm_min` (`machine/mod.rs:250-252`; then the
  RAW and COMMANDED axes are one axis, and the F-2 notes in
  `feeds/mod.rs:1940-1996` collapse), and `FeedsDerates::safety_factor`
  (`feeds/mod.rs:610`, `:701`, `:2610`).
- The power job becomes a machine fact: the power budget is
  `power_at_rpm(rpm)`, with no fraction, at every site listed in 1.4
  (R4 class 2, "limits, not scales"). The dial's power target
  `P <= k x P0` is the margin. Q2 asks if a separate continuous-duty
  fraction is wanted.
- The plunge and ramp ship at the material base
  (`Material::plunge_rate_base`, `feeds/mod.rs:2342`) and the ball tip cap
  (`:2361-2371`). A plunge is full-width axial, so the dial has no lever on
  it (Q5).
- It does not move tonight.

### 3.2 The L/D de-rate 0.88 / 0.75 (S8): stays as a minor cut, made visible

- **Today no warning, no diagnostic id and no face row carries it.** It
  shows only inside the "Derated x... by ..." sentence
  (`ui/feeds/why.rs:267`) and in the explore hover reason "tool overhang"
  (`ui/feeds/explore.rs:334-335`). The inspector rule is "a warning behind a
  hover is a warning that was deleted" (`ui/properties/feeds_speeds.rs:231-235`).
- **It is not minor on a default tool.** `ToolConfig::new_default` sets
  `stickout: 45.0` (`compute/tool_config.rs:219`). Every tool of 7.5 mm or
  less then has L/D > 6 and takes 0.75. On the matrix the de-rate fires on
  470 of 498 cells.
- **The record that WP1 adds:**
  - `FeedsWarning::LongToolDerate { stickout_mm, diameter_mm, ratio, factor }`
    in `feeds/mod.rs` (the enum at `:713`), pushed in the L/D block
    (`:1901-1912`) when `ld_factor < 1.0`;
  - the id `ids::FEEDS_LONG_TOOL_DERATE = "feeds.long_tool_derate"` in
    `diagnostics/ids.rs` (near `:109`) and in `ids::ALL` (`:327-339`);
  - the arm in `diagnostics/adapters/from_feeds.rs` (Severity `Info`,
    Confidence `Static`, text "Long tool: stickout 45.0 mm is 7.1 x D; feed
    x0.75 (repo rule, unsourced)");
  - the arm in `ui/feeds/why.rs::warning_lines` (`:436`), which
    `why::draw_warnings` paints on the inspector card face
    (`ui/properties/feeds_speeds.rs:238`).
- The exact UI row is the warning line under the inspector card, drawn by
  `crate::ui::feeds::why::draw_warnings`.
- It keeps its value tonight. With the 0.75 factor gone, the L/D cut alone
  still puts 140 banded cells under the band minimum (1.3). Q7 asks whether
  WP5 bounds it at the band minimum or moves it into the dial.

### 3.3 The rigidity factors (P4 and S3b)

- `RigidityProfile` (`machine/mod.rs:53-62`) carries two default sets that
  disagree by 20-33 % with no source (5.1-12, 3.3-14). `depth_cap_mm`
  (`:114-141`) is the one producer of the roughing depth cap. It clamps on
  168 shipping cells.
- They are limits on the base engagement, not scales on the chip. The dial
  acts after them (2.3).
- R2 owns the depth cap. R2's smaller step (no depth ceiling on finishing
  passes; deflection decides) proceeds under its own spec. This spec does
  not move a rigidity factor.
- They stay named as "rule of thumb, no published source" in the
  `RigidityDepthCap` doc (`machine/mod.rs:64-72`).

### 3.4 The workholding factor 0.85 / 1.03 (S9)

- It is a feed scale in the same class as the 0.75 factor
  (`feeds/mod.rs:1913-1922`). It fires on 0 matrix cells (Medium), and on
  every Low or High project.
- Under the ruling a margin acts only through the dial. Recommendation: WP5
  moves it into the dial's target, `k_eff = k x w` with `w = 0.85` for Low
  and `w = 1.00` for Medium and High. The 1.03 raise goes (Q8).

### 3.5 The drill multiplier 2.5 (S1)

- R1 refuses every drill cell in the four judged woods:
  `feeds::support::formula_backing` (`feeds/support.rs:133-138`) returns
  `Clueless` for every tool family on `Drill`.
- It remains live for the materials that the judgement did not cover:
  `PlywoodSoftwood`, `Hdf`, `Particleboard`, `Acrylic`, `Hdpe`,
  `Polycarbonate`, `Delrin`, `Aluminum`, `Fiberglass`
  (`feeds/vendor_lut.rs:114-137`).
- Fate: hold the value, flag it. The `FormulaOnly` source string already
  names it unsourced (`feeds/support.rs:42-47`). WP6 adds a Caution on a
  shipped drill recipe. It is not an engagement scale, so the dial does not
  touch it.

### 3.6 The rubbing floor (S13): the warning stays, the clamp goes

- The lift sites:
  - `feeds::calculate` Step 9b (`feeds/mod.rs:2464-2497`), which calls
    `effective_rubbing_floor` (`:1100`) and `rubbing_floor_clamp_reason`
    (`:1115`);
  - Suggest pass 9 (`suggest/adaptive_entry.rs:447-490`), which re-applies
    the lift and files `SuggestWarning::FeedClampedToChiploadFloor`
    (`feeds/suggest.rs:610`; rationale `feeds/rationale.rs:578`).
- The report-only readers stay: `feeds/efficiency.rs:253` and the modulation
  note (`dressup/feed_modulation.rs:111-113`, `:565`). The modulator reads
  the band minimum, not the floor.
- The machinery that exists only to explain a lift (WP2b):
  `ClampReason` (`feeds/feed_explanation.rs:106-163`),
  `recipe_parked_by_rubbing_floor` (`feeds/mod.rs:1154`),
  `CommandedStage::clamped_to`, and the (c2) "clamped, not exceeds" rule in
  `tool_load/chipload.rs:655-690` and `tool_load/verdict.rs:1479-1489`.
  With no lift, no Suggest recipe parks on the band ceiling.
- The warning threshold stays `effective_rubbing_floor(band)` =
  `min(0.025, band max)` in WP2. That needs no new ruling. Q9 asks whether
  the threshold becomes `min(0.025, band min)`, so that a vendor band below
  0.025 (4.1-26) outranks the unsourced constant.
- The warning keeps the name `FeedsWarning::ChiploadClampedToFloor` only
  if the text changes. Recommended rename:
  `FeedsWarning::ChiploadBelowRubbingFloor { commanded, floor, band_max }`,
  and `SuggestWarning::FeedBelowChiploadFloor`. The id
  `feeds.chipload_clamped_to_floor` becomes `feeds.chipload_below_floor`.
- **The sentry `rubbing_floor_never_exceeds_band` becomes
  `rubbing_floor_warns_and_never_lifts`** (file
  `crates/rs_cam_core/tests/rubbing_floor_warns_and_never_lifts.rs`):
  - test 1 (B3 sub-floor fixture): the warning fires; the commanded advance
    equals `derates.effective_chip_load_mm()` to 1e-9 (no lift); and it is
    at or below the band maximum;
  - test 2 (the Ø6.35 oak ball control): no warning, as today;
  - `feeds/CLAUDE.md` names the new sentry in place of the old one.
- The effect tonight: the 80 floor cells drop. The drop ratio is 0.42 to
  0.98, median 0.70. 18 cells ship under 0.0125 mm/tooth. Review cell 3
  (Ball d3.175 Scallop hardwood) goes from 875 to about 434 mm/min. Each of
  the 80 cells carries the Caution.

### 3.7 `SuggestAggressiveness` (Conservative / Default / Speed)

- `SuggestAggressiveness` (`feeds/suggest.rs:70-96`) and
  `SuggestPolicy::aggressiveness` (`:131`) are inert on the pre-simulation
  path since 2026-08-13 (`feeds/suggest/tests.rs:2255-2292`). No code
  outside tests reads `policy.aggressiveness`.
- It is a chipload position in the band. The ruling fixes the chipload at
  the band midpoint times the ladder. WP3 deletes the enum, the field, the
  reserved `RationaleReason::SpeedTargetGated` (`feeds/rationale.rs:131`),
  and the three tests that pin it. This removes the second meaning of the
  word "aggressiveness".

### 3.8 The machine ceiling (S11)

- It is a machine fact and stays a limit. It binds on 95 cells and pushes
  77 banded cells under the band (1.3).
- To keep the chip in the band, the RPM can follow the feed down, as the
  drill envelope already does (`feeds/mod.rs:2540-2562`), bounded by the
  row `rpm_min` and the machine minimum. WP5 does this if the operator
  agrees (Q10).

### 3.9 The power ladder's feed rung (rung 4)

- Rung 4 (`feeds/mod.rs:2240-2278`) cuts the feed last when the RPM, the
  depth and the width cannot fit the spindle. It is a response to a machine
  limit, not a margin. It stays. It fires on 0 matrix cells.
- It already reports its factor (`PowerLadderReducedCut::feed_factor`).

## 4. Migration order

Each package re-runs the FM1 instrument
(`scripts/cargo_lane.sh test -p rs_cam_core -q --test feeds_matrix_instrument_fm1 -- --ignored --nocapture`)
and the named sentries only. No full heavy gate (operator ruling
2026-09-11). Every cargo command goes through `scripts/cargo_lane.sh`.

### WP1. The long-tool de-rate becomes visible (SAFE TONIGHT)

- **Numbers:** none move.
- **Files:** `crates/rs_cam_core/src/feeds/mod.rs` (variant and push);
  `crates/rs_cam_core/src/diagnostics/ids.rs` (id and `ALL`);
  `crates/rs_cam_core/src/diagnostics/adapters/from_feeds.rs` (arm);
  `crates/rs_cam_core/src/diagnostics/tests.rs` (`:117` id list);
  `crates/rs_cam_viz/src/ui/feeds/why.rs` (`warning_lines` arm);
  `crates/rs_cam_core/src/feeds/CLAUDE.md` (sentry line).
- **Sentries:** add `long_tool_derate_is_shown_ld1` in
  `crates/rs_cam_core/tests/`: the warning fires if and only if
  `derates.ld_overhang < 1.0`, and it carries the ratio and the factor.
  Add a viz check that `why::draw_warnings` paints the line (extend
  `the_recommendation_explains_each_row_g_whyrow`). Re-run FM1; the
  `diagnostic_ids` column gains `feeds.long_tool_derate` on 470 cells.
  Run `every_cell_declares_its_feeds_support_fm0` (arms must not move).
- **FEATURE_CATALOG.md** (Machine and material models, the feeds row): "The
  long-tool de-rate (x0.88 above 4 x D stickout, x0.75 above 6 x D; repo
  rule, unsourced) shows as an Info line on the Feeds card and as
  `feeds.long_tool_derate` in diagnostics and MCP."
- **Risk:** low. Noise: the line fires on almost every default tool
  (stickout 45 mm). A snapshot or count sentry of diagnostics can go red;
  re-bless it with this package as the cause.

### WP2. The rubbing floor warns and does not lift (SAFE TONIGHT)

- **Numbers:** the 80 floor cells drop (3.6). Nothing rises.
- **WP2a files:** `crates/rs_cam_core/src/feeds/mod.rs` (Step 9b: keep the
  test, drop the lift; rename the warning); `feeds/suggest/adaptive_entry.rs`
  (pass 9: same); `feeds/suggest.rs` and `feeds/rationale.rs` (the
  `SuggestWarning` variant and its entry);
  `diagnostics/adapters/from_feeds.rs` and `diagnostics/ids.rs` (text and
  id); `crates/rs_cam_viz/src/ui/feeds/why.rs:520-545` (text: "Advance per
  tooth below the rubbing floor: 0.012 mm/tooth (floor 0.025, repo rule,
  unsourced). The feed is not raised.").
- **WP2b files (optional tonight, safe):** delete `ClampReason`,
  `recipe_parked_by_rubbing_floor`, `CommandedStage::clamped_to`, and the
  (c2) arm in `tool_load/chipload.rs` and `tool_load/verdict.rs`.
- **Sentries to rewrite or retire:**
  - `rubbing_floor_never_exceeds_band` becomes
    `rubbing_floor_warns_and_never_lifts` (3.6);
  - `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`: retire the Step 9b and
    pass 9 arms; keep the Step 9c drill envelope arm;
  - `_litmatrix_rubbing_floor_clamp`, `rubbing_floor_envelope_band_p1`,
    `rubbing_floor_diameter_scaling_measurement`,
    `ceiling_advisory_and_clamp_record_a7`, `chipload_boundary_g_chip_ulp`:
    re-bless the lift assertions as "warns, feed unchanged";
  - `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs`
    asserts the floor headline verbatim: update the text;
  - `wanaka_suggest_integration` takes minutes: ask first.
  - Re-run FM1; FM0 arms must not move.
- **FEATURE_CATALOG.md:** "The rubbing floor (0.025 mm/tooth, repo rule,
  unsourced) is a Caution only. Suggest no longer raises a feed to it
  (ruling R4, 2026-09-23)."
- **Risk:** medium. The feeds drop on small ball and end-mill cells, and
  those recipes can burn. Part of the drop reverses at WP3: with the 0.75
  factor gone, only 45 of the 80 cells stay under the floor (33 if the L/D
  cut also goes). The Caution states it on each cell. The package is
  large in test count, not in logic.

### WP3. The dial ships and the 0.75 factor leaves (NEEDS THE OPERATOR'S LOOK)

One package, one push. The commits inside it, in order:

1. **Rename only.** `MachineProfile::safety_factor` becomes
   `aggressiveness` at every site (48 files outside `planning/`, among them
   10 `test_data/*.toml` and 4 `crates/rs_cam_core/tests/fixtures/*.toml`
   that carry `safety_factor = 0.75`). Same value, same uses. Every number
   is byte-identical; the sentries stay green without a re-bless.
2. **Split the two jobs.** The power budget reads `power_at_rpm` alone at
   every site in 1.4. Step 9 stops multiplying feed, plunge and ramp.
   `commanded_cutting_feed_ceiling_mm_min` returns the cutting ceiling.
   `FeedsDerates::safety_factor` goes. Pass 6b (2.3) lands in
   `feeds/suggest/aggressiveness.rs` with its warning, rationale reason and
   diagnostic id. This is the commit that moves numbers.
3. **Surfaces.** Machine panel, why, explore, compare, explain payload, MCP
   `inspect_machine`, CLI text (2.6).
4. **Delete `SuggestAggressiveness`** (3.7).
5. **Docs.** `feeds/CLAUDE.md`, `machine/CLAUDE.md`, FEATURE_CATALOG.md,
   `planning/PROGRESS.md`.

- **Sentries to add:**
  - `aggressiveness_holds_the_chip_and_cuts_the_load_r4a`: for a grid of
    roughing cells, the shipped chip equals the dial-1.00 chip; the force
    and the power at the shipped point are at or below `k` times the base;
  - `aggressiveness_never_cuts_the_feed_r4b`: at every `k`, the shipped
    advance per tooth does not fall; a floor-stopped solve files
    `target_met: false`;
  - `aggressiveness_075_loads_no_cut_above_today_r4c`: on the FM1 grid, the
    force at dial 0.75 is at or below the force of the pre-WP3 recipe
    (2.4).
- **Sentries to re-bless** (cause: R4): `power_ceiling_parity_f2`,
  `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
  `a_published_power_is_at_the_depth_that_cuts_g_s2`,
  `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`,
  `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`,
  `a_criterion_carries_its_own_bound_g_s4bound`,
  `the_depth_that_cut_is_a_measured_load_g_s3depth`,
  `machine::tests::commanded_cutting_ceiling_is_the_ceiling_after_the_safety_factor`,
  `machine::tests::test_safety_factor_range`, `feeds::tests::test_safety_factor_applied`,
  `one_depth_derate_for_feed_and_band_fm3`, FM0 and the FM1 re-run, and
  the viz sentries `the_speeds_apply_holds_the_cut_g_speedsonly` and
  `the_recommendation_explains_each_row_g_whyrow`.
- **FEATURE_CATALOG.md:** "Machine aggressiveness (0.50-1.00, default
  0.75) sets the load of a Suggest recipe as a fraction of full engagement.
  The chipload stays at the vendor band midpoint times the published depth
  ladder; Suggest makes the depth per pass and the stepover smaller. The
  hidden x0.75 feed factor is gone. The spindle power ceiling is the rated
  power curve."
- **Risk:** high. Feeds rise by 1.33 on 453 cells; depths and stepovers fall
  on every roughing cell; the power gate reads a larger budget; the
  optimizer (`tool_load/optimize`) and the feed modulator see a larger
  budget too. Pass counts rise, so generation time and air time rise.
  Existing projects that were applied under 0.75 do not change until the
  operator presses Suggest again.

### WP4. (Reserved: R2 depth caps.) Not in this spec.

### WP5. The remaining feed scales join the band rule (AFTER WP3, NEEDS RULINGS)

- Q7: L/D bounded at the band minimum, or moved into the dial target.
- Q8: workholding into the dial target.
- Q10: RPM follows the feed down when the machine ceiling binds.
- Files: `feeds/mod.rs` Step 5b and Step 7; `suggest/aggressiveness.rs`.
- Risk: medium; each moves numbers on its own cell set.

### WP6. A Caution on the drill multiplier (SAFE, NOT REQUESTED)

- A Caution `feeds.drill_multiplier_unsourced` on every shipped drill recipe
  (non-judged materials). No number moves.

## 5. Open questions for the operator

Each has a recommendation.

1. **Q1. The default dial value.** Recommendation: 0.75. At 0.75 no cut
   loads more than today, the chip returns to the band, and a depth-only cut
   removes material at the same rate (2.4). A default of 1.00 means no
   margin.
2. **Q2. The power ceiling after the 0.75 goes.** Recommendation: the
   rated power curve, `power_at_rpm`, with no fraction. The dial's `P`
   target is the margin. The alternative is a named machine field
   `continuous_power_fraction` (repo margin, unsourced, shown on the panel).
3. **Q3. Values above 1.00.** Recommendation: refuse. Above 1.00 the dial
   would need an engagement larger than the base, and the base comes from
   unsourced default factors (S3b).
4. **Q4. The dial on finishing passes.** Recommendation: no action on the
   Finish role. The finish depth is the stock allowance and the width is the
   scallop target. R2's deflection check is the finish limiter.
5. **Q5. Plunge and ramp.** Recommendation: ship the material base with no
   factor. The dial has no engagement lever on a plunge.
6. **Q6. The word "aggressiveness".** The simulation modulator already has
   `modulation_aggressiveness` (`dressup/feed_modulation.rs:270`,
   `session/compute.rs:1547`, the CLI flag from `rs_cam_cli/src/main.rs:217`).
   Recommendation: rename it to `modulation_feed_scale` in a later package,
   so one word has one meaning.
7. **Q7. The L/D cut and the band.** Tonight it stays a feed cut (WP1 makes
   it visible). With the 0.75 gone it still puts 140 banded cells under the
   band minimum. Recommendation: move it into the dial target as a load
   fraction (`k_eff = k x ld`), because a long tool bends more under force,
   and force is what the dial holds. Second choice: keep it on the feed,
   bounded below by the band minimum.
8. **Q8. Workholding.** Recommendation: into the dial target, Low 0.85,
   Medium and High 1.00; delete the 1.03 raise.
9. **Q9. The floor warning threshold.** Recommendation: `min(0.025, band
   min)`. A published band minimum outranks the unsourced constant (4.1-26).
   WP2 keeps `min(0.025, band max)` until this ruling.
10. **Q10. The machine ceiling.** Recommendation: let the RPM follow the feed
    down to hold the chip, bounded by the row `rpm_min` and the machine
    minimum. This puts the 77 ceiling cells back in the band.
11. **Q11. How depth and width share the cut.** The ruling says "not depth
    alone". Recommendation: one common scale on depth and width for every
    roughing and semi-finish family, Adaptive included (2.3). The
    alternative is a depth-weighted split (for example `ap = s^a x ap0`,
    `ae = s^(1-a) x ae0` with `a` near 1), which keeps more removal rate.
    A width-first order is not an option: the edge term makes the width a
    weak lever on force.
