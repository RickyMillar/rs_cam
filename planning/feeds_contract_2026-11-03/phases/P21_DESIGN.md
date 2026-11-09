# P21 design: the constant-chip spindle strategy (2026-11-09)

Design note only. No code changed. HEAD `90e03440b`. No cargo run, no GUI run, no MCP read.
Every number in this note comes from a file in the repo, from arithmetic shown here, or from an
`rg` count over `crates/` at HEAD. The lead verifies each phase; agents implement and never
commit.

Inputs read: `NEXT_SESSION.md` section 2 (rulings of 2026-11-08), `acceptance/wanaka_21_header.md`,
`acceptance/derive_wanaka_21.py`, `acceptance/wanaka_21.md`, `phases/P20.md` sections 6 and 7,
`phases/P20i.md`, `crates/rs_cam_core/src/feeds/CLAUDE.md`, `crates/rs_cam_core/src/machine/CLAUDE.md`.

## 0. Summary

1. P21 replaces `SpindleStrategy::MaxSpeed` with `ConstantChip`. The chip per tooth holds. One RPM per op rises to the first named limit.
2. The limits are spindle max, the op cutting ceiling (XY), the Z axis rate (drill family only), power at that RPM, acceleration and the operator ceiling.
3. `MAX_SPINDLE_SPEEDUP` 1.5, `SPINDLE_CEILING_HEADROOM` 0.95, the five milling RPM tiers and `step2b_milling_tier` go. The F201 Unsourced pin falls from 152 to 142.
4. With `VfdConstantTorque` below the rated RPM, required and available power both scale with n. Power cannot bind there. It can bind above the rated RPM and on `ConstantPower`.
5. No acceleration rule for an RPM exists in the code. Section 2.4 gives three candidates. The operator must rule (question 1). P21 phase P21d carries it.
6. `ConstantChip` refuses without stated axis rates and accelerations. The refusal uses the existing Suggest refusal door. Generation and export do not change.
7. The operator ceiling is one project field, `[job.post] spindle_rpm_ceiling`. Empty means no rise above the chart RPM. A row with no printed RPM then has no RPM basis.
8. The default stays `MatchChart`. Thus no FM1 cell moves, and the SFM rule that gives 10610 stays on the `MatchChart` path (question 6).
9. Under P21, with wanaka on `constant_chip` and a ceiling of 24000, 16 of the 18 failing rpm rows PASS. Idx 5 and 9 have no Suggest RPM today.
10. The `arc_fit_disposition_a5` arm returns to 2499 only when its DC-2 fixture opts into `ConstantChip` with stated rates and a ceiling. The XY rate then binds at 19419 rpm.

## 1. Survey

### 1.1 Writers of the RPM inside the calculator

The step order is in `feeds/calculator.rs:250-265`. Each row writes `Speed::rpm` and its basis.

| Site | Step | What it writes | Constant it uses |
|---|---|---|---|
| `feeds/calculator/speed.rs:98` `step1_rpm` | 1 | `clamp_rpm(Vc x 1000 / (pi x d_eng))` | `material.base_cutting_speed_m_min.*` (Unsourced; solid wood 200 at `material/mod.rs:742`); `FALLBACK_RPM` 18000 (Unsourced, `feeds/mod.rs:1594`) for d_eng <= 0 |
| `speed.rs:137` `step2_vendor_rpm` | 2 | `clamp_rpm(row.rpm_nominal)` when the row prints one | none (printed) |
| `speed.rs:160` `step2b_max_speed` | 2b | under `MaxSpeed`: `rpm x min(ceiling / rpm, 1.5)` | `MAX_SPINDLE_SPEEDUP` 1.5 (`mod.rs:350`, F201 row `mod.rs:1711`); `SPINDLE_CEILING_HEADROOM` 0.95 (`mod.rs:355`, no F201 row); tiers via `milling_rpm_ceiling_for_diameter` (`mod.rs:1697`); `row.rpm_max` |
| `speed.rs:234` `step2b_milling_tier` | 2b' | under `MaxSpeed`, not Drill, not V-bit or tapered ball: `min(rpm, tier)` | `MILLING_CEILING_TIERS` (4 x 2 rows) and `MILLING_CEILING_LARGE_RPM` (`mod.rs:1600-1660`), all Unsourced |
| `calculator/power.rs:212` `rung1_rpm` (write at `:309`) | 6, rung 1 | the RPM down the constant-chip line until the power fits, only where a slower RPM lowers the load | `machine.power_at_rpm` |
| `calculator/feed.rs:190` `step7_feed_ceiling` (write at `:267`) | 7 | arm L of Q1: `next_rpm_at_or_below(floor(ceiling / chip per rev))` | `machine.cutting_feed_ceiling_mm_min()` |
| `calculator/close.rs:184` `step9c_drill` (write at `:278`) | 9c | the drill RPM follows the drill feed down to the Z axis rate | `VerticalRateLimit` (`feeds/vertical.rs:149`) |

`SpindleScaleReason` (`mod.rs:941`) names the last mover: `Unchanged`, `MaxSpeedPolicy`,
`PowerLimit`, `FeedCeiling`. `FeedsDerates::spindle_scale` holds the product of the moves.

### 1.2 Readers of the RPM and of the strategy, outside the calculator

| Site | What it reads | Constant |
|---|---|---|
| `gcode/mod.rs:1283` `PostConfig::spindle_speed` (default 18000 at `:1358`) | the project default S word | none |
| `gcode/mod.rs:1296` `PostConfig::spindle_strategy` | the project policy | none |
| `compute/catalog.rs:2346` `effective_spindle_rpm` | op RPM, else `post.spindle_speed` | none; export, simulation, cycle time and diagnostics read it |
| `session/mod.rs:2306` `suggest_slots`; `:2334` `cutter_op_profile` (strategy at `:2356`); `:2368` `safe_envelope` (strategy at `:2383`) | the one Suggest context (F114) | none |
| `feeds/suggest/apply.rs:137`, `:256`, `:573` | the Apply funnel writes `spindle_rpm` (`Command::ApplyFeeds`, `session/command.rs:574`) | none |
| `session/mutation/toolpath.rs:1005` | `seed_feeds` copies a suggested RPM on a tool change | none |
| `session/compute/params.rs:354` | `set_toolpath_param spindle_rpm` (manual) | none |
| `feeds/envelope.rs:794-846` `calc_at_with` | one calculation per grid point, with the strategy | none |
| `feeds/operating_point.rs:370` `power_at_shipped_point` | the card, MCP and CLI power figure at the shipped point | none |
| `session/mutation.rs:154` `post_change_reaches_motion` | exempts `spindle_strategy` from invalidation (WP17) | none |
| `session/command.rs:484` `set_spindle_strategy` (row `SetPostConfig`) | the one mutation of the strategy | none |
| `rs_cam_mcp/src/server.rs:348` `SpindleStrategyParam`; `:3001` `SetSpindleStrategyParam` | the wire token `match_chart`, `max_speed` | none |
| `rs_cam_viz/src/mcp_server.rs:564` `get_suggest_rationale`; `:1539` `set_spindle_strategy` | MCP doors | none |
| `rs_cam_viz/src/app/mcp/commands.rs:701` | MCP handler of the strategy | none |
| `rs_cam_cli/src/main.rs:91` `SpindleStrategyArg`; `:415` `--apply-suggest`; `:431` `--spindle-strategy` | CLI doors | none |
| `rs_cam_cli/src/project.rs:826`, `:906`, `:1369` | the strategy override and the Apply door | none |
| `rs_cam_viz/src/ui/properties/feeds_speeds.rs:1081` `draw_spindle_policy_row` | the GUI combo; its hover prints both constants (`:1101-1102`) | `SPINDLE_CEILING_HEADROOM`, `MAX_SPINDLE_SPEEDUP` |
| `feeds_speeds.rs:260-367` (RPM row), `:464` (manual write through `FeedsLock`), `:605` (range note) | the card RPM row | none |
| `rs_cam_viz/src/ui/feeds/why.rs:193-210` | the sentence of `SpindleScaleReason` | none |
| `rs_cam_viz/src/ui/feeds/shared.rs:285` | the Explore preview writes the recommended RPM into a scratch op | none |
| `rs_cam_viz/src/ui/properties/pills.rs`, `ui/feeds/compare.rs:116`, `state/runtime.rs:303` | pass the strategy into a calculation | none |
| `machine/mod.rs:1276` `rpm_range`; `:1260` `power_at_rpm`; `:1003` `cutting_feed_ceiling_mm_min`; `:1020` `cutting_ceiling`; `:1044` `op_cutting_ceiling`; `:1069` `z_axis_rate_mm_min` | the machine limits | none |
| `machine/mod.rs:697`, `:888` `ControllerSettings::rpm_max`, `rpm_range` | the firmware `$30`; a PWM spindle only | none |
| `machine/kinematics.rs:93` `MachineKinematics`; `:1395` `predicted_achieved_feed`; `:1423` `trapezoidal_peak_velocity` | the reach of one move | none |
| `dressup/feed_modulation.rs:1338-1380` | the modulator's reach probe at `band.max x lift x rpm x Z` | none |
| `tool_load/verdict.rs:44` `BindingConstraint::KinematicReach` | the per-move reach cap | none |

### 1.3 Unsourced constants on the RPM path

| Constant | Site | Value | P21 |
|---|---|---|---|
| `MAX_SPINDLE_SPEEDUP` (+ `MAX_SPINDLE_SPEEDUP_ROW`) | `feeds/mod.rs:350`, `:1711` | 1.5 | delete |
| `SPINDLE_CEILING_HEADROOM` | `feeds/mod.rs:355` | 0.95 | delete (no F201 row) |
| `MILLING_CEILING_TIERS` | `feeds/mod.rs:1600-1652` | 3 mm 22000, 6 mm 20000, 8 mm 18000, 10 mm 16000 | delete |
| `MILLING_CEILING_LARGE_RPM` | `feeds/mod.rs:1655` | 14000 | delete |
| `material.base_cutting_speed_m_min.*` (8 rows) | `material/mod.rs:742-781` | solid wood 200, plywood 180, sheet good 170, plastic 250, aluminium 100, foam 300, fibreglass 120, custom 200 | keep on `MatchChart`; not read by `ConstantChip` (section 3) |
| `FALLBACK_RPM` | `feeds/mod.rs:1594` | 18000 | keep (guard for d_eng <= 0) |

The rows that leave F201: 1 (`MAX_SPINDLE_SPEEDUP_ROW`) + 8 (tiers) + 1 (large) = 10. The pin
`PINNED_UNSOURCED_COUNT` (`tests/every_feeds_formula_constant_states_its_source_f201.rs:103`)
goes from 152 to 142, on purpose, in the behaviour phase.

### 1.4 Tests that pin an RPM or the old strategy

| Test | Pin | Under P21 |
|---|---|---|
| core lib `feeds::tests::spindle_strategy_max_speed_lifts_rpm_and_scales_feed` (`feeds/tests.rs:1719`) | speedup > 1.05, or rpm >= 0.99 x 24000 x 0.95; `spindle_scale` within 0.05 of the speedup | rewrite (P21c) |
| core lib `feeds::tests::spindle_strategy_max_speed_respects_hard_cap` (`:1807`) | speedup <= 1.5 | delete (P21c) |
| core lib `test_lut_rpm_override_within_machine_range` (`:1407`) | rpm 18000 +/- 100 | no move (`MatchChart`) |
| core lib `test_no_lut_backward_compatible` (`:1440` area) | 6000 < rpm < 24000 | no move |
| core lib `test_drill_family_rpm_is_the_surface_speed_rpm` (`:512`), `test_vbit_rpm_uses_engaged_diameter` (`:824`), `test_flat_tool_rpm_chipload_unchanged_by_engaged_d` (`:879`) | the SFM RPM `Vc x 1000 / (pi d)` | no move under the recommended design; move under question 6 (b) |
| core lib `test_drill_envelope_ceiling_drops_rpm_to_hold_chipload` (`:636`) | the drill follow-down | no move |
| `_litmatrix_milling_rpm_diameter_tier` (6 tests, `:96-151`) | 12 mm <= 14000, 10 mm <= 16000, 8 mm <= 18000, 6 mm <= 20000, 3 mm <= 22000, 1.5 mm <= 22000 | delete (P21c); the tiers go |
| `_litmatrix_rpm_only_lut_chipload` (`:64`, `:86`) | rpm > 0 under `MaxSpeed` and `MatchChart` | `MaxSpeed` arm: re-point to a machine with stated rates (P21c) |
| `_litmatrix_ipe_janka_scaling` (`:100`) | directional chip, under `MaxSpeed` | re-point as above |
| `literature_matrix` (shim `tests/literature_matrix/shim.rs:492`, every cell under `MaxSpeed`) | 50 cells with an `[cell.expected.rpm]` band; blocks at major severity | re-point the shim to `MatchChart`; measure (P21c) |
| `rpm_follows_the_feed_ceiling_fm8` (`:335`) | rpm `floor(4000 / (0.254 x lift))`, reason `FeedCeiling` | no move |
| `the_envelope_keeps_each_limit_k2b` (`:102`) | `rpm / spindle_scale` = the chart RPM | no move under `MatchChart`; P21c keeps `spindle_scale = n / n_chart` |
| `a_vbit_row_is_read_at_its_printed_key_b4` (`:252`, `:368-370`), `a_flat_end_plunge_is_the_side_chip_over_z_g6` (`:307`) | `rpm_nominal` 18000 and the shipped 18000 | no move |
| `wanaka_suggest_integration` (`:759-774`) | a drill ships the chart RPM | no move while wanaka stays `match_chart`; moves to 19685 if the project switches (question 7) |
| `arc_fit_disposition_a5` `retired_lift_leaves_feed_at_the_calculator_value` (DC-2, `:441-475`) | 2499 (known red: 2317) | section 6.3 |
| `save_keeps_the_simulation_wp17` (`:282`) | toggles `MatchChart` and `MaxSpeed`; no invalidation | rename; add the ceiling (P21b) |
| viz `the_panel_feeds_run_once_per_input_f216` (`:298`, `:351`, `:413`) | toggles the strategy | rename (P21b) |
| viz `controller::tests::post_event::the_spindle_strategy_event_writes_the_session_f072` | `MaxSpeed` | rename (P21b) |
| cli `main.rs:933` round trip of `SpindleStrategyArg` | every variant has a token | rename (P21b) |
| mcp wire snapshot and `an_unknown_mcp_field_is_refused_f26` | the param schema | re-bless (P21b) |
| `every_feeds_formula_constant_states_its_source_f201` | 152 | 142 (P21c) |
| `gcode/mod.rs:2264-2991`, `gcode/wizard_overlay.rs:586-803`, `tests/common/k5_cases.rs:121` | the literal S10610 in emit tests | no move (an emit fixture, not a Suggest value) |
| `machine/mod.rs:1388` | `clamp_rpm(10610)` = 10000 on a discrete spindle | no move |

FM1 (`feeds_matrix_instrument_fm1`, cells in `tests/common/feeds_cells.rs:174`) builds every cell
with `SpindleStrategy::default()` = `MatchChart`. Section 6.4 gives the effect.

## 2. The constant-chip rule as math

Symbols: fz the target chip per tooth (Step 2, the chart row), Z the flute count, lambda the
radial lift at the shipped width (Step 5, `Feed::radial_lift`, `geometry::RadialThinning`), n the
RPM. The feed is

    F(n) = fz x Z x lambda x n

The chip per rev `c = fz x Z x lambda` is fixed once Step 5 has run. The rule is

    n_cc = min(n_spindle, n_xy, n_z, n_power, n_acc, n_op)

and the shipped RPM is `next_rpm_at_or_below(floor(n_cc))` (a whole rev, as Step 7 does today).
The feed is `floor(c x n)`. The card names the limit that gives the minimum. No multiplier enters.

The rule only rises. If `n_cc` is at or below the RPM after Step 2, the RPM stays, and Step 7
(Q1 arms) acts as today. Thus a `ConstantChip` op whose limits all lie below the chart RPM is
byte-identical to `MatchChart`.

The rule needs fz and lambda, so it runs after Step 5. It must run before Step 6, because the
power check reads n. Its place is a new step, "Step 5c: constant-chip RPM", between
`step5b_long_tool` and `step6_power` (`calculator.rs:282-284`).

### 2.1 Spindle max

    n_spindle = machine.rpm_range().1

A discrete spindle takes the highest listed speed at or below `n_cc`.

### 2.2 XY: the op cutting ceiling

    n_xy = C / (fz x Z x lambda)

C is `MachineProfile::op_cutting_ceiling(feed_max_rate)` (P20e: THE cut ceiling of one op). Its
`CuttingCeilingSource` names the limit on the card: the stated cutting cap, the kinematics X/Y
rate, or the op `feed_max_rate`. The travel-rate source cannot occur, because `ConstantChip`
needs stated rates (section 4). For the Adaptive family lambda is at the cap width (ruling P3 b).

### 2.3 Z: the vertical rate (drill family only)

A drill cycle has one feed, and it is vertical:

    n_z = v_z / (fz_axial x Z)

v_z is `machine.z_axis_rate_mm_min()` (`$112`). fz_axial is the G6 axial chip
(`vertical::AxialChipBound`). On wanaka: 2500 / (0.0635 x 2) = 19685.04, floored 19685.

A side op does not take n_z. Its plunge and the vertical part of its ramp already go through
`VerticalRateLimit::bound` (P20b), which caps the plunge at v_z at any n. The acceptance script
uses the same rule (`derive_wanaka_21.py` `n_b_lateral`: spindle max and XY only).

### 2.4 Acceleration on the modulated moves

No rule in the code sets an RPM from the acceleration. The code has these parts:

- `machine/kinematics.rs:1423` `trapezoidal_peak_velocity`: a move of length L reaches
  `v_peak^2 = a L + (v_in^2 + v_out^2) / 2`, clamped to the cruise ceiling.
- `kinematics.rs:1395` `predicted_achieved_feed` and `predicted_feeds_for_toolpath`: the reach
  per move, through `digest_moves` and `walk_junctions`.
- `dressup/feed_modulation.rs:1338`: the modulator probes the reach at `band.max x lambda x n x Z`
  and caps a move at it (`BindingConstraint::KinematicReach`).
- P20c: the gates grade the reached feed (`use_predicted_feed_in_gates`).

The effect on the RPM: on move i the reached chip is `min(F(n), r_i) / (n Z lambda_i)`, with
r_i the reach of the move. Where `r_i < F(n)`, the chip falls as n rises. A higher n thus makes
the short moves rub. The reach r_i does not depend on n when the probe runs at the cutting
ceiling, because the feed moves no geometry. So one probe per toolpath serves every n.

Three candidates. Each needs a generated toolpath; Suggest reads no simulation (Q5), and the
reach needs no simulation either.

- **A1 (time share).** n_acc is the largest n where the cutting moves that reach F(n) hold at
  least s of the cutting time. The share s is a new number with no source.
- **A2 (chip floor).** n_acc = min over the gate population of `r_i / (chip_min x Z x lambda_i)`,
  with chip_min the printed band minimum. No new number. But one short move sets the minimum.
  At a = 1200 mm/s^2 a 0.05 mm move reaches sqrt(1200 x 0.05) = 7.7 mm/s = 465 mm/min. With
  chip_min 0.127 and Z 2 that gives 1831 rpm, below the 6000 spindle minimum. A2 then always
  binds unless a population rule drops short moves, and that rule is a new number.
- **A3 (advisory).** No cap. The card states the share of the cutting time that reaches F(n) at
  the chosen n, and a finding warns when the share falls. This holds the "never refuse a job"
  rule, but it does not meet the ruling, which names acceleration as a cap.

Recommendation: ship A3 in P21c as a named limit that reports and does not cap. Make it a cap in
P21d after the operator rules (question 1). Before generation, the limit shows "not judged: no
toolpath".

### 2.5 Power at that RPM

The pre-sim power is the maximum over the `PowerCases` of the cut (K7). For one case, P20d gives

    P_req(n) = [Ks ae ap F(n) + F_edge ap pi D n Z psi / (2 pi)] / 6e7
             = n x [Ks ae ap c + F_edge ap D Z psi / 2] / 6e7
             = k x n

Both terms are linear in n at a fixed chip, so the maximum over the cases is `k_max x n`.

`VfdConstantTorque { rated_power_kw P_r, rated_rpm n_r }` (`machine/mod.rs:1260`):

    P_avail(n) = P_r x min(n, n_r) / n_r

- For n <= n_r: `P_req / P_avail = k_max n_r / P_r`, a constant R. Power cannot set n here. If
  R > 1 at the chart RPM, it is R > 1 at every n <= n_r; the Step 6 ladder lowers depth or width,
  as today (rung 1 does not lower n, because a slower n does not help).
- For n > n_r (a spindle whose max is above the rated RPM): P_avail = P_r, so
  `n_power = P_r / k_max = n_r / R`. It binds when it is below the other limits.

`ConstantPower { power_kw P }`: `n_power = P / k_max`. The preset `generic_wood_router`
(0.8 kW, `machine/mod.rs:1123`) is this case.

On wanaka n_r = 24000 = spindle max, so power never binds a constant-chip rise. That agrees with
the ruling and with the acceptance header ("Power never sets the RPM").

### 2.6 The operator ceiling

    n_op = spindle_rpm_ceiling,           when the field is set
         = n after Step 2 (the chart RPM), when the field is empty and the row prints an RPM
         = no basis,                      when the field is empty and the row prints no RPM

The third line is the refusal of section 4 with its own reason. An empty field thus gives the
`MatchChart` RPM on every printed row, and the rise starts only when the operator sets a value.

### 2.7 The binding limit on the card

A typed enum `RpmLimit` (no wildcard arm): `SpindleMax`, `CuttingCeiling(CuttingCeilingSource)`,
`ZAxisRate`, `PowerAtRpm`, `Acceleration`, `OperatorCeiling`, `ChartRpm`. `FeedsResult` gains
`rpm_choice: Option<RpmChoice>` (`None` under `MatchChart`) with the shipped n, the binding
limit and every limit with its RPM. `PointBases::rpm` becomes `Basis::derived("constant chip to
the first limit", [chip, the binding limit's basis])`. `SpindleScaleReason::MaxSpeedPolicy`
becomes `ConstantChip`; `spindle_scale` stays `n / n_chart`.

Later: one "N % of the theoretical limit" setting multiplies the binding limit's RPM and the card
states it. P21 leaves a seam for it in `RpmChoice` and adds no value.

## 3. The removed constants and the 10610

| Constant | What replaces it |
|---|---|
| `MAX_SPINDLE_SPEEDUP` 1.5 | nothing; the rise stops at a named limit |
| `SPINDLE_CEILING_HEADROOM` 0.95 | nothing; the spindle max is the limit. An operator who wants margin sets the ceiling |
| the milling RPM tiers (22000 to 14000 by diameter) | the operator ceiling. The tiers claimed Onsrud, Amana and GWizard bands; no file in the repo prints them (`MILLING_CEILING_CLAIM`) |
| `row.rpm_max` as a cap in `step2b_max_speed` | question 9: a printed vendor RPM maximum is a source, so it can stay as a named limit `VendorRpmMax` |

The 10610 on wanaka idx 5 and 17. The Onsrud hardwood row prints a chip only (`ONS61`). Step 2
finds no `rpm_nominal`, so Step 1 stays: 200 m/min x 1000 / (pi x 6 mm) = 10610 rpm.
`material.base_cutting_speed_m_min.solid_wood` is Unsourced (`SPEED_CLAIM_PORT`).

After P21, under `ConstantChip`, the RPM of that op comes from the chip row and the limits only:
`n_cc = min(spindle max, XY, power, acceleration, ceiling)`. The SFM value is not read. With the
wanaka ceiling at 24000: n_xy = 15000 / (0.14919 x 2 x 1) = 50271, so the spindle max binds and
n = 24000. With an empty ceiling, the row has no chart RPM, so the RPM has no basis (section 4).

Under `MatchChart` the SFM path stays. It serves 187 shipping FM1 cells whose row prints no RPM
and 28 formula-only cells (section 6.4). Removing it changes all of them; that is question 6.

`[job.post] spindle_speed = 10610` in the wanaka file is the project default S word. No code
writes it (`rg "spindle_speed = " crates` finds only test and CLI job fixtures). Every wanaka op
states its own `spindle_rpm`, so `effective_spindle_rpm` never reads it there. P21 does not touch
it. The operator can set it to any value (question 10).

## 4. The refusal

Rule: `ConstantChip` gives no RPM when the machine states no axis rates or no accelerations.
"Stated" means `kinematics.max_rate_xyz_mm_min` is `Some` and `kinematics.acceleration_xyz_mm_s2`
is `Some` (question 3: whether the scalar `acceleration_mm_s2` alone counts). Every built-in
preset has `kinematics: None` (`machine/mod.rs:566`), so every preset refuses.

The second case is section 2.6: an empty ceiling on a row with no printed RPM.

Door: a new `FeedsError` variant, `ConstantChipUnstated { missing: ConstantChipGap }`, with
`ConstantChipGap` = `AxisRates`, `Accelerations` or `RowPrintsNoRpm`. The existing refusal surfaces
show it:

- the GUI card: the basis headline "Refused" with the reason, as for `Unbacked` (the card already
  draws it; no new text block);
- MCP `get_suggest_rationale`: `basis` {headline "refused", detail};
- CLI `suggest` and `project --apply-suggest`: the refusal line;
- `Command::ApplyFeeds`: writes no feed field. A suggested RPM field clears, a manual one stays
  (the `seed_feeds` rule, `session/CLAUDE.md`).

The refusal is at the Suggest level only. Generation and export run with the op's stored RPM, or
`post.spindle_speed` (`effective_spindle_rpm`). No job is refused (the no-refusal rule).

Consequence: the feed is `c x n`, so with no n the feed, the plunge and the ramp have no basis
either. A `FeedsError` hides the chip too. Question 2 asks whether the card must still show the
chip; that needs an `Option` RPM in `FeedsResult`, a much larger change.

## 5. The operator ceiling field

- **Place**: `gcode::PostConfig::spindle_rpm_ceiling: Option<u32>`, beside `spindle_strategy`.
  It is an operator policy, not a machine fact; the spindle max is the machine fact (question 4).
- **File key**: `[job.post] spindle_rpm_ceiling = 24000`. `serde(default, skip_serializing_if =
  "Option::is_none")`, so an old file loads and saves as it was.
- **Engine type**: `SpindleStrategy::ConstantChip { rpm_ceiling: Option<u32> }` (the enum stays
  `Copy`). `PostConfig` keeps two flat keys; one accessor `PostConfig::spindle_policy()` builds the
  engine value. The 132 files that write `SpindleStrategy::default()` do not change.
- **Invalidation**: add the field to the exempt list of `post_change_reaches_motion`
  (`session/mutation.rs:154`) and to the per-field rule of `mutation/config.rs:649`. Suggest reads
  it live.
- **Domain**: a value outside the spindle range is refused at every door (F210 rule, sentry
  `a_value_outside_its_domain_is_refused_at_every_door_w6b`).
- **GUI**: one `DragValue` "RPM ceiling" in the same row as the "Spindle policy" combo
  (`feeds_speeds.rs:1081`). It is enabled only under "Constant chip". Empty shows a grey "chart".
  The RPM row gets one chip with the binding limit ("24000 - spindle max"); the hover lists every
  limit with its RPM. The hover text loses both deleted constants.
- **MCP**: `set_spindle_strategy` takes `{strategy: "match_chart" | "constant_chip",
  rpm_ceiling: u32 | null}`. A break, no alias (MCP crate rule); add a `build_info().features`
  token. `get_suggest_rationale` gains `basis.rpm` {value, binding, limits: [{limit, rpm}]}.
- **CLI**: `project --spindle-strategy constant_chip --spindle-rpm-ceiling 24000`; `suggest`
  prints the binding limit. The job-file key is the same as the project key.

## 6. Predicted effect

### 6.1 The rpm rows of the acceptance table

Condition: the wanaka file states `spindle_strategy = "constant_chip"` and
`spindle_rpm_ceiling = 24000` (the operator approves that edit), and the acceleration limit does
not cap (A3, section 2.4). The wanaka machine states both rates and accelerations
(`nz_wanaka_21_operator.toml:76-88`), so it does not refuse.

The table has 18 rpm rows, all FAIL today (`wanaka_21.md` summary), not 16. Sixteen have a
Suggest RPM; idx 5 and 9 print none and are graded on the shipped value.

| idx | Expected (script) | App today | P21 value | Binding | Grade after P21 |
|---|---|---|---|---|---|
| 0 | 24000 | 18000 | 24000 | spindle max (n_xy 32605) | PASS |
| 1 | 24000 | 18000 | 24000 | spindle max (n_xy 32800) | PASS |
| 2 | 24000 | 18000 | 24000 | spindle max (n_xy 41330) | PASS |
| 3 | 19685 | 18000 | 19685 | Z rate | PASS |
| 4 | 19685 | 18000 | 19685 | Z rate | PASS |
| 5 | 24000 | none (shipped 10610) | none in Suggest | - | FAIL until Suggest prints an RPM for this op, then PASS after an apply (question 13) |
| 6 | 24000 | 18000 | 24000 | spindle max | PASS |
| 7 | 24000 | 18000 | 24000 | spindle max | PASS |
| 8 | 24000 | 18000 | 24000 | spindle max | PASS |
| 9 | 24000 | none (shipped 18000) | none in Suggest | - | FAIL, as idx 5 |
| 10 | 24000 | 18000 | 24000 | spindle max | PASS |
| 11 | 24000 | 18000 | 24000 | spindle max | PASS |
| 12 | 24000 | 18000 | 24000 | spindle max | PASS |
| 13 | 24000 | 18000 | 24000 | spindle max | PASS |
| 14 | 24000 | 18000 | 24000 | spindle max | PASS |
| 15 | 24000 | 18000 | 24000 | spindle max | PASS |
| 16 | 24000 | 18000 | 24000 | spindle max (n_xy 59055) | PASS |
| 17 | 24000 | 10610 | 24000 | spindle max (n_xy 42182) | PASS |

Predicted totals: PASS 47 + 16 = 63; FAIL 23 - 16 = 7 (idx 5 rpm, idx 9 rpm, idx 6, 7, 8
plunge 240, idx 16 chip, idx 16 feed); NEEDS-RULING 28; N/A 10. If the acceleration limit caps
below 24000, those rpm rows fail again, because the script grades no acceleration limit
(question 8).

### 6.2 Feed, plunge and ramp rows

These rows are graded at the app's own RPM, so a feed that scales with n keeps its grade. The
values change as follows (whole mm/min, floored):

| idx | Quantity | Today | P21 | Grade after |
|---|---|---|---|---|
| 0, 12 | feed / ramp | 6488 / 6447 | 8650 / 8596 | NEEDS-RULING (UNSOURCED chip) |
| 1 | feed / ramp | 6999 / 6448 | 9332 / 8597 | NEEDS-RULING |
| 2 | feed / plunge / ramp | 5279 / 2285 / 4571 | 7038 / 2500 (Z) / 6096 | PASS |
| 3, 4 | drill feed = plunge | 2286 | 2499 (Z 2500) | PASS |
| 6 | feed | 1010 | 1346 | NEEDS-RULING |
| 7, 8 | feed (8: ramp too) | 990 | 1320 | NEEDS-RULING |
| 10, 13 | feed | 2286 | 3048 | as today |
| 11 | feed | 6488 | 8650 | NEEDS-RULING |
| 14 | feed / ramp | 8060 / 6448 | 10746 / 8597 | NEEDS-RULING |
| 15 | feed / plunge / ramp | 5429 / 2285 / 4571 | 7238 / 2500 / 6096 | PASS |
| 16 | feed | 2317 | 3089 | FAIL (chip 0.06437 against 0.0762; graded at 24000: P4a 3658, P4b 6096) |
| 17 | feed / plunge / ramp | 3165 / 1347 / 3165 | 7161 / 2500 / 7161 | PASS |

Tool 6 plunges (idx 0, 1, 11, 12, 14) rise from 2162 to min(2162 x 24000 / 18000, 2500) = 2500;
the bracket is 0-2500 UNSOURCED, so NEEDS-RULING holds. The idx 6, 7, 8 plunge 240 does not scale
with n and stays FAIL. Power ratios do not change (section 2.5). No feed reaches 15000.

### 6.3 `arc_fit_disposition_a5`

The DC-2 fixture (`arc_fit_disposition_a5.rs:441-475`) runs on `MachineProfile::default()` with
`max_feed_mm_min` 2500 and no kinematics, under the default `MatchChart`. P21 does not change
that path, so the arm stays at 0.06437 x 2 x 18000 = 2317 (red).

The arm returns to 2499 only when the fixture opts in: `ConstantChip { rpm_ceiling: Some(24000) }`,
`kinematics.max_rate_xyz_mm_min = [2500, 2500, z]` and stated accelerations. Then
n = floor(2500 / (0.06437 x 2)) = 19419 and F = floor(0.12874 x 19419) = 2499 (lift 1: a finish
pass does not thin). The 2499 then comes from the XY rate at 19419 rpm, not from 24000 rpm as on
2026-09-24. P21c re-points the fixture and records the new reason in the pin comment.

### 6.4 FM1

FM1 builds every cell with `SpindleStrategy::default()` (`tests/common/feeds_cells.rs:174`), and
P21 does not change `MatchChart`. Prediction: no FM1 cell moves, and the matrix is not re-blessed.

If the operator rules question 6 (b) (delete the SFM path on `MatchChart` too), the cells that
read it move: 187 shipping cells whose matched row prints no RPM (plywood 56, hardwood 55,
softwood 44, MDF 32) and 28 formula-only cells (20 ball, 8 V-bit). Counts: a script over
`matrix_2026-09-23.csv` and the `rpm_nominal` of each row in `data/vendor_lut/observations/`.

## 7. Phases (one agent each)

Blast radius at HEAD (`rg -c` over `crates/`, files in brackets): `SpindleStrategy` 410 (132),
`MaxSpeed` 61 (17), `MAX_SPINDLE_SPEEDUP` 17 (5), `SPINDLE_CEILING_HEADROOM` 7 (5),
`milling_rpm_ceiling` 17 (2), `MILLING_CEILING` 17 (1), `SpindleScaleReason` 19 (7),
`spindle_scale_reason` 11 (8), `FeedsError` 121 (28), `op_cutting_ceiling` 56 (20),
`power_at_rpm` 51 (17), `z_axis_rate_mm_min` 4 (3), `base_cutting_speed` 24 (4).

### P21a: one RPM-choice door (byte-identical)

- Move Steps 2b and 2b' into one function `calculator/spindle.rs::rpm_choice` that returns
  `RpmChoice { rpm, binding: RpmLimit, limits }`. The old caps enter as named limits for one phase
  (`LegacySpeedup`, `LegacyHeadroom`, `LegacyTier`).
- Add `FeedsResult::rpm_choice` (`None` under `MatchChart`).
- Sentries: new `every_rpm_cap_is_a_named_limit_p21` (the shipped RPM equals the minimum of the
  named limits, under `MaxSpeed`, on a 3, 6, 10 and 12 mm grid). Run: f201, fm8, k2b,
  `_litmatrix_milling_rpm_diameter_tier`, core `--lib feeds`, `literature_matrix`.
- Predicted pins: none move. FM1 byte-identical (no re-bless).
- Blast: `speed.rs`, `calculator.rs`, `feeds/mod.rs` (3 files).

### P21b: the strategy rename and the ceiling field (byte-identical on `MatchChart`)

- Rename `MaxSpeed` to `ConstantChip { rpm_ceiling }` in core, MCP, CLI and viz (61 refs, 17
  files). File token `constant_chip`; `max_speed` is refused on load with a message that names the
  new key (38 planning TOMLs hold it; no fixture under `planning/fixtures` or `crates/`).
- Add `PostConfig::spindle_rpm_ceiling`, the WP17 exemption, the domain check, the GUI field,
  the MCP param field and the CLI flag. In this phase the engine does not read the ceiling.
- Sentries moved: `save_keeps_the_simulation_wp17` (add the ceiling), viz f216 and f072, cli
  round trip, the mcp wire snapshot (re-bless), `an_unknown_mcp_field_is_refused_f26`, w6b.
- Predicted pins: the MCP snapshot (schema break, stated in the commit). No numeric pin.
- Blast: 17 files for the rename, about 15 readers of `post_config().spindle_strategy`.

### P21c: the constant-chip rule (the behaviour step)

- Step 5c with the limits of sections 2.1-2.3, 2.5 and 2.6; acceleration as the A3 report.
- The refusal of section 4 (`FeedsError::ConstantChipUnstated`); each exhaustive `match` on
  `FeedsError` names the variant (121 refs, 28 files; most are tests that build one variant).
- Delete `MAX_SPINDLE_SPEEDUP`, `SPINDLE_CEILING_HEADROOM`, the tiers, `step2b_milling_tier` and
  the legacy limits of P21a. F201 pin 152 to 142.
- `SpindleScaleReason::ConstantChip`; `why.rs` sentence; card binding chip; MCP `basis.rpm`; CLI
  line.
- New sentries:
  - `constant_chip_rises_to_the_first_named_limit_p21`: a wanaka-like machine gives 24000 (spindle
    max) on a side op and 19685 (Z) on a drill; a small XY rate binds `CuttingCeiling`; a
    `ConstantPower` machine binds `PowerAtRpm` at `P / k`; a VFD with n_r below the spindle max
    binds at `n_r / R`; a ceiling binds `OperatorCeiling`; an empty ceiling gives the chart RPM.
  - `constant_chip_refuses_without_machine_rates_p21`: a preset refuses with the named gap; the
    op still generates and exports.
  - `no_rpm_multiplier_survives_p21`: a source scan (as f26) finds none of the deleted names.
- Pins that move, with the reason:
  - `spindle_strategy_max_speed_lifts_rpm_and_scales_feed`: rewrite on a machine with stated
    rates and a ceiling; assert the chip holds and the binding limit.
  - `spindle_strategy_max_speed_respects_hard_cap`: delete; the 1.5 cap is gone.
  - `_litmatrix_milling_rpm_diameter_tier` (6 tests): delete; the tiers are gone. The new sentry
    covers "the cap is named".
  - `_litmatrix_rpm_only_lut_chipload`, `_litmatrix_ipe_janka_scaling`: re-point the
    `ConstantChip` arm to a machine with stated rates; their assertions are directional.
  - `literature_matrix`: the shim moves to `MatchChart` (presets refuse `ConstantChip`). The 50
    rpm bands must be measured; a cell that reaches major severity blocks. Not predictable here.
  - `arc_fit_disposition_a5` DC-2: re-point the fixture (section 6.3); 2499 holds again.
  - f201: 152 to 142.
- No FM1 re-bless (section 6.4).
- Docs: `feeds/CLAUDE.md` (invariant and sentries), `FEATURE_CATALOG.md` (the strategy).

### P21d: the acceleration limit as a cap (after question 1)

- One rule from section 2.4, on the generated toolpath, with the reach probe at the cutting
  ceiling. Sentry `the_acceleration_limit_reads_the_reach_of_the_moves_p21d`.
- Predicted pins: the P21c sentry gains an `Acceleration` arm; wanaka rpm rows can move (question 8).

### P21e: the wanaka re-grade (lead or a grading agent)

- The operator approves the TOML edit (`constant_chip`, ceiling 24000). Check that the GUI binary
  was built from HEAD. Read MCP, update `APP` in `derive_wanaka_21.py`, re-run, and compare with
  section 6.1.
- `wanaka_suggest_integration` (`:763`, the drill ships the chart RPM) moves to 19685 with the
  file edit. Ask before the run (it takes minutes).

## 8. Risks and open questions for the operator

1. **Acceleration rule.** A1 (a time share with a new number), A2 (the band minimum, which one
   short move sets), or A3 (report, no cap). Recommendation: A3 now, a cap after the ruling.
2. **Refusal shape.** A full Suggest refusal hides the chip as well as the RPM and the feed. To
   show the chip, `FeedsResult::rpm` must become optional, a larger change. Recommendation: the
   full refusal.
3. **"Stated accelerations".** Per-axis `$120-$122` only, or the scalar `acceleration_mm_s2` too?
   Recommendation: per-axis only; the `$$` import fills it.
4. **Ceiling place.** Project post block (recommended), the machine profile, or per tool? A per-tool
   ceiling would stand where the deleted diameter tiers stood.
5. **Empty ceiling on a row with no printed RPM.** No basis (recommended), or the SFM RPM?
6. **The 10610 on `MatchChart`.** (a) Keep the Unsourced SFM on `MatchChart` and label it on the
   card (recommended; no FM1 move). (b) Delete it; 187 + 28 FM1 cells move or lose their RPM.
7. **Wanaka file.** The acceptance rows pass only after the file states `constant_chip` and a
   ceiling of 24000. The operator approves that edit.
8. **Acceptance script.** `derive_wanaka_21.py` grades no acceleration limit. If the P21d cap binds,
   the script needs the same rule, or those rows fail.
9. **Vendor `rpm_max`.** A printed RPM maximum is a source. Keep it as a named limit
   `VendorRpmMax`? Recommendation: yes; 71 rows print one.
10. **`[job.post] spindle_speed = 10610`.** No code writes it and no wanaka op reads it. The operator
    may set it to 18000 or leave it.
11. **Step 7 and the op ceiling.** Step 7 reads `cutting_feed_ceiling_mm_min()` without the op
    `feed_max_rate`; the new XY limit reads `op_cutting_ceiling(feed_max_rate)` (P20e). Recommendation:
    Step 7 reads the op ceiling too, in P21c, so one ceiling serves both.
12. **`$30` = 1000.** The wanaka controller block states `rpm_max = 1000`. If the firmware holds that
    value, an S24000 command is scaled or clamped. The operator checks `$30` before a
    constant-chip job.
13. **Idx 5 and 9.** Suggest prints no RPM for these ops today (the power is not modelled). The
    constant-chip RPM needs no depth. Should Suggest print it there? Recommendation: yes, in P21c.
14. **Reach and rubbing.** A higher n raises the modulator's band-ceiling probe. More moves then
    meet `KinematicReach`, and the chip gate (reached feed, P20c) can report more rubbing. The cycle
    time can fall less than n rises. A3 reports this share.
15. **Heat at high surface speed.** At 24000 rpm an 18 mm tool runs at pi x 18 x 24000 / 1000 =
    1357 m/min. The deleted tiers were the only guard, with no source. After P21 the operator
    ceiling is the guard.
16. **Old `max_speed` files.** 38 research TOMLs under `planning/adaptive_research_2026-10-23/`
    hold `max_speed`. Refuse on load (recommended, the no-alias rule), or map to `constant_chip`
    with an empty ceiling?
17. **Envelope grid.** Under `ConstantChip` each grid point has its own n, because lambda changes
    with ae. The plot then shows an RPM that varies over the plane. The pick writes one RPM.

## 9. Lead rulings (2026-11-09, on section 8)

Settled by the lead from the rulings on record (no underived defaults; the no-alias rule; one
definition; the ceiling is an operator policy):

- Q2 refusal shape: the full Suggest refusal. The chip is still readable in the basis detail.
- Q3 "stated accelerations": the per-axis `$120-$122` values only. The `$$` import fills them.
- Q4 ceiling place: the project post block, `[job.post] spindle_rpm_ceiling`.
- Q5 an empty ceiling on a row with no printed RPM: no basis (the refusal with its own reason).
- Q8 the acceptance script grades the acceleration limit with the same rule in the P21d commit;
  until then the rows are graded with A3 (no cap).
- Q9 a printed vendor `rpm_max` is a named limit `VendorRpmMax`.
- Q11 Step 7 reads `op_cutting_ceiling(feed_max_rate)` (one ceiling for both), in P21c.
- Q13 Suggest prints the constant-chip RPM on idx 5 and 9 (no depth needed), in P21c.
- Q16 `max_speed` refuses on load with a message that names `constant_chip`. The 38 research
  TOMLs are evidence files; a migration script runs only on the ones a test loads (list them in
  the P21b note).
- Q14, Q15, Q17 are risks, not decisions: the A3 share report (Q14) and the operator ceiling
  (Q15) are the guards; the envelope plot states "RPM varies with ae" in one chip (Q17).
- Order: P21a and P21b (byte-identical) start after P22-0c-5 lands; one implementer in the tree
  at a time. P21c waits for the operator answers to Q1, Q6 and Q7 below.

For the operator (asked 2026-11-09 evening): Q1 the acceleration rule (A1 / A2 / A3-first),
Q6 the unsourced SFM RPM on `MatchChart` ((a) keep and label, (b) delete), Q7 + Q10 the wanaka
file edit (`constant_chip`, ceiling 24000; `spindle_speed` 10610 -> 18000 or left), Q12 `$30`.

Operator answers (2026-11-09 evening, in the operator's words, read by the lead):
- Q1: "not sure what you are asking. but you can get the accel from the machine profile?" The
  value is in the profile; the open point is the RULE that turns it into an RPM cap. Lead: A3
  (report the reached share, no cap) ships in P21c; the operator sees the share on wanaka before
  any cap rule (P21d) is chosen.
- Q6: "10k is low. It makes me feel like it might be wrong anyway. If something is there to
  replace it we can remove." Lead reading: delete the SFM RPM where the constant-chip rule can
  replace it; where no limit can be judged (a preset with no stated rates) the row has no RPM
  basis and says so. The FM1 cells that lose the SFM RPM are listed in the P21c note before the
  re-bless (section 6.4 counts).
- Q7/Q10: "I don't know enough to say. Don't make anything specific to this project; it must be
  a generic implementation. I'm not even sure what constant chip is." Lead: the policy and the
  ceiling are fields every project has (nothing wanaka-specific in code). The wanaka file edit
  waits for a plain-words yes; the acceptance rows that need it are listed in section 6.1.
- Q12: "$30 is 24000 on the machine; the file is wrong." The wanaka controller block `rpm_max`
  is corrected to 24000 (lead edit, 2026-11-09).

Operator answers (2026-11-10 morning, in the operator's words):
- Q7/Q10: "yes, try constant chip on wanaka." The wanaka file takes `spindle_strategy =
  "constant_chip"` and `spindle_rpm_ceiling = 24000` (the lead edits it through the GUI door and
  saves). Under P21b the engine still runs the legacy caps; the ceiling binds in P21c.
- Q1: "not sure." Open. A3 (the reached share, no cap) ships in P21c as ruled; the lead shows the
  operator the share on wanaka before any cap rule.
- Q6 stands as read (delete the SFM RPM where the constant-chip rule replaces it).
P21c is therefore unblocked after P22b.
