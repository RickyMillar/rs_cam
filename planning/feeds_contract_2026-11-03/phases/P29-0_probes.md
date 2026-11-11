# P29-0: measurement probes

Measured 2026-11-11 on HEAD `484542abe`. No product code changed. No commit.

Status: complete. The probe test is deleted; `git status` for `crates/` is clean. The raw
tables and the probe source are in `phases/P29-0_raw/`.

## 1. Method

- One temporary test file, `crates/rs_cam_core/tests/p29_0_probes.rs`. It prints tables and
  asserts nothing. It is deleted. A copy is `phases/P29-0_raw/p29_0_probes.rs.txt`.
- The probe loads each project with `ProjectSession::load`. It saves nothing.
- Projects: `planning/rivmap_block_job/nz_wanaka_21_operator.toml` ("operator", 18 mm roughs)
  and `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml` ("work", 6 mm roughs).
- "idx" is the position in the toolpath list (the P28 numbers). The toolpath ids differ:
  idx 5 = id 11, idx 6 to 8 = id 12 to 14, idx 9 = id 15, idx 10 = id 16, idx 11 = id 17,
  idx 12 = id 18, idx 13 = id 19, idx 14 = id 20, idx 15 = id 21, idx 16 = id 23, idx 17 = id 24.
- Every load value comes from an existing model function. The probe defines no load.

Commands (each through the lane script, in the foreground):

```
P29_PROJECT=operator timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -j 2 --test p29_0_probes -- --ignored presim --nocapture
P29_PROJECT=work     timeout 300 scripts/cargo_lane.sh test -p rs_cam_core -j 2 --test p29_0_probes -- --ignored presim --nocapture
```

Each pre-simulation run took 22 s (debug build).

## 2. Pre-simulation: the force of the stated cut against the budget

Code path of each column:

| Column | Code path |
|---|---|
| Budget | `machine.rigidity.force_budget_n` = 35.32 N on both files |
| ap, ae | `OperationConfig::cut_spec().calc_inputs(D)` (`compute/catalog/cut_spec.rs:335`). Where it gives no depth, the probe reads `Axial::stated_mm()` and marks the value "dropped". |
| fz | the stated feed / (the stated rpm x Z) |
| Mean and peak force | `Engagement::from_tool_cut(CutPoint{ap, ae}, tool).edge_force(line, fz, Z)` (`feeds/engagement.rs:211, 348`) |
| Budget depth | `feeds::force_budget::depth_at_budget(tool, material, ae, fz, budget)` (`feeds/force_budget.rs:351`) |
| Swept width | `PowerCases::of_operation(op).full_feed_width(ae, D)` (`feeds/engagement.rs:888`): the pass ceiling `c D` of a geodesic rough |
| Deflection | `feeds::predict::tip_deflection_from_engagement` (`feeds/predict.rs:624`) at the same engagement |
| Power | `feeds::power_at_operating_point` (`feeds/operating_point.rs:240`) |
| Rule | `RigidityProfile::depth_cap_rule(family, role)` (`machine/mod.rs:293`) |

### 2.1 The adaptive roughs (item 3 of the brief)

| File | idx | Tool | Stated ae x ap (mm) | fz (mm) | Mean N | Peak N | Peak / budget | Budget depth at the stated width (mm) | Swept width (mm) | Peak N at the swept width | Budget depth at the swept width (mm) | Rule today |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| operator | 1 | 18 mm 2F | 5.5 x 10.0 | 0.2354 | 43.9 | 145.1 | 4.11 | 2.09 | 5.99 | 149.6 | 2.06 | Factor 2 x D |
| operator | 2 | 6 mm 2F | 1.5 x 6.0 | 0.1466 | 17.4 | 45.3 | 1.28 | 3.59 | 1.64 | 48.3 | 3.44 | Factor 2 x D |
| operator | 14 | 18 mm 2F | 3.6 x 4.166 | 0.2239 | 12.6 | 57.7 | 1.63 | 2.46 | 3.94 | 59.0 | 2.40 | Factor 2 x D |
| operator | 15 | 6 mm 2F | 1.382 x 5.532 | 0.1508 | 15.3 | 43.3 | 1.23 | 3.70 | 1.51 | 46.2 | 3.52 | Factor 2 x D |
| work | 1 | 6 mm 2F | 1.2 x 6.0 | 0.1536 | 15.0 | 39.5 | 1.12 | 3.95 | 1.31 | 42.1 | 3.75 | Factor 2 x D |
| work | 2 | 6 mm 2F | 1.5 x 6.0 | 0.1425 | 17.2 | 44.7 | 1.27 | 3.67 | 1.64 | 47.6 | 3.51 | Factor 2 x D |
| work | 14 | 6 mm 2F | 1.2 x 3.0 | 0.1536 | 7.5 | 29.2 | 0.83 | 3.95 | 1.31 | 30.0 | 3.75 | Factor 2 x D |
| work | 15 (disabled) | 6 mm 2F | 1.382 x 5.532 | 0.1463 | 15.0 | 42.7 | 1.21 | 3.78 | 1.51 | 45.5 | 3.60 | Factor 2 x D |

Result against section 5 of `P29_FINDINGS.md`:

- "18 mm idx 1 at 5.5 x 10 mm is about 4 x over": confirmed. The value is 4.11 x at the stated
  width and 4.24 x at the swept width.
- "6 mm idx 1: depth 6.0 to about 3.85 mm at 1.2 mm width": confirmed in size. The exact value
  is 3.95 mm at the stated width and the stated chip, and 3.75 mm at the swept width. 3.85 mm
  is between the two.
- "idx 14 (3 mm) no change": confirmed for the work file (0.83 x). On the operator file idx 14
  is 1.63 x over; its depth falls from 4.17 mm to about 2.4 mm.
- "idx 2 not measured": measured. It is 1.27 to 1.28 x over on both files. Its depth falls from
  6.0 mm to about 3.5 to 3.7 mm.

### 2.2 The slot and groove ops (item 2 of the brief, pre-simulation part)

| File | idx | Op, tool | ap (mm) | ae | fz (mm) | Mean N | Peak N | Peak / budget | Budget depth (mm) | Deflection (um) | Rule today |
|---|---|---|---|---|---|---|---|---|---|---|---|
| both | 5 | Project Curve, 6 mm 2F flat | 4.0 (the offset; `calc_inputs` drops it) | slot, 6.0 | 0.1492 | 40.6 | 51.2 | 1.45 | 2.70 | 19.7 | none (Trace, Finish) |
| both | 9 | Project Curve, 3.175 mm 20 degree V-bit, 1F | 2.418 (the offset; dropped) | slot, 3.175 | 0.1270 | 11.3 | 15.4 | 0.44 | 5.54 | 34.3 | none |
| both | 10 | Trace, 6.35 mm 60 degree V-bit, 2F | 2.6 | slot, 6.35 | 0.0635 | 18.1 | 20.4 | 0.58 | 4.51 | 3.6 | none |
| both | 13 | Trace, 6.35 mm 60 degree V-bit, 2F | 0.6 | slot, 6.35 | 0.0635 | 4.2 | 4.7 | 0.13 | 4.51 | 1.0 | none |

The values are the same on both files: the chip per tooth of these ops is the same (the feed
and the RPM scale together).

- idx 5 is the only slot op over the budget before the simulation: 1.45 x, when the 4 mm offset
  is read as a depth. The budget depth is 2.70 mm.
- The model reads the V-bit ops through `Engagement::from_tool_cut`, which takes the engaged
  diameter of the cone at the depth. The width "slot = D" is the nominal diameter; the arc
  clamps to a full slot.
- The pre-simulation power of idx 5 and idx 9 is `DepthAfterSimulation`: no figure.

### 2.3 The other roughing ops, for reference

| File | idx | Op, tool | ae x ap (mm) | Peak N | Peak / budget | Budget depth (mm) |
|---|---|---|---|---|---|---|
| both | 0, 12 | Face, 18 mm | 8.0 x 0.5 | 7.5 | 0.21 | 2.37 |
| both | 11 | Pocket, 18 mm | 8.0 x 0.5 | 7.5 | 0.21 | 2.37 |
| operator | 6 | Pocket, 2 mm 1F | 0.8 x 0.7 | 5.3 | 0.15 | no root below the flute |
| operator | 7, 8 | Pocket, 2 mm 1F | 1.0 x 0.5 | 4.3 | 0.12 | no root below the flute |
| work | 7, 8 | Pocket, 2 mm 1F | 1.0 x 0.5 | 3.9 | 0.11 | no root below the flute |
| both | 17 | Profile, 6 mm (slot) | 6.0 x 1.25 | 16.6 | 0.47 | 2.70 |

These values are at the stated width. The slot ring of a Pocket (ae = D) is not in this table;
the pre-simulation power figure names it (`slot, ae 2.00` and `slot, ae 18.00`), the force does
not.

## 3. Post-simulation: method

Commands (the operator gave the go for runs over three minutes):

```
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -j 2 --profile release-fast --test p29_0_probes --no-run        (3 min 39 s)
P29_PROJECT=operator P29_TARGETS=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,17 timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -j 2 --profile release-fast --test p29_0_probes -- --ignored postsim --nocapture   (4 min 12 s)
P29_PROJECT=work     P29_TARGETS=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,17    timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -j 2 --profile release-fast --test p29_0_probes -- --ignored postsim --nocapture   (8 min 15 s)
```

- The probe generates every enabled op but the scallop finish (idx 16) in the order of
  `generation_plan::plan(Scope::Project)`, with the plan's simulation steps before each rest op,
  then one closing `ProjectSession::run_simulation`. idx 15 is disabled in the work file.
- Simulation options: the product defaults (`SimulationOptions::default()`: feed modulation on,
  the gates read the predicted feed), at the project cell of 0.2 mm.
- A calibration run showed why the full run is necessary: idx 10 simulated alone cut fresh
  stock (22.6 mm deep, 122 N). After the earlier ops it reads 4.6 mm and 36 N.
- "Removing sample" = a sample of the op with `!SimulationCutSample::is_air()`.
- "Steady" = `tool_load::locality::is_steady_state_for_gate` is true (the span filter).
- Kinds: "entry" = `is_configured_entry`; "phantom" = `is_phantom_transit`, named by the span
  kind in the sample's ancestry and by `source_intent`.

Code path of each value:

| Value | Code path |
|---|---|
| Feed filter | `sample.feed_rate_mm_min >= 0.95 x ctx.steady_feed` (`tool_load/chipload.rs:197, 225-245`); the constant is read from the source; the steady feed is `ProjectSession::load_context_parts(idx).steady_feed` |
| Effective feed | `tool_load::effective_feed_for_sample` (`tool_load/mod.rs:76`) |
| Power | `tool_load::power::sample_power_kw` at the effective feed; limit `MachineProfile::power_at_rpm` |
| Peak and mean force | `tool_load::deflection::sample_engagement(tool, s).edge_force(line, fz, Z)` with `fz = deflection_feed_per_tooth_mm`. This is the body of `cut_force::sample_peak_force_n`; one call gives the mean too. |
| Deflection | `tool_load::deflection::sample_tip_deflection_mm`; limit `deflection::exceeds_limit(pass_role, leave)` |
| Chip | `tool_load::display::peak_chip_per_tooth` with `RadialThinning::of_lut_role(pass_role)`; band `tool_load::chip_target_for_toolpath` |
| Cap source | The `FeedCap` items of the semantic trace (`SemanticKey::CapSource`), as the P22a readout reads them |
| Verdicts | `ProjectSession::build_tool_load_report(Some(trace))` |
| M5 force at the gate's cut | `feeds::force_budget::peak_force_n(tool, material, s.axial_engagement_mm, Engagement::from_sample(s).ae_mm(), effective feed / (rpm x Z))`: the cut that `tool_load/depth.rs:222-288` solves the cap at. "Over the budget" at this cut is the inverse of "deeper than `depth_at_budget`". |

Two limits of the readout:

- The count "chip over the band maximum" is a plain `>` with no tolerance. Every such sample
  equals the band maximum to four decimals (the modulator holds the chip at the maximum). The
  gate's own verdict is `Within` on every op. The count is not used below.
- The trace holds the feed of the simulation run. On an op that a later simulation step did
  not run again after its modulation, the sample feed is the feed before the modulation, and
  the effective feed is the feed after it. The filter in the code reads the sample feed. The
  table gives both.

## 4. M1: the 95 % feed filter of the chip gate

Share of the steady removing samples that the filter drops. "Chip under" = samples with a peak
chip below the band minimum (a band minimum exists on idx 5 and idx 9 only).

| File | idx | Op | Steady removing samples | Dropped by the filter (sample feed) | Dropped (effective feed, all removing) | Chip under, with filter | Chip under, no filter | Chip median with / without | Chip verdict today (population) |
|---|---|---|---|---|---|---|---|---|---|
| operator | 1 | 3D Adaptive 18 mm | 45 027 | 41 172 (91.4 %) | 91.2 % | 0 | 0 | 0.153 / 0.105 | Within |
| operator | 2 | 3D Adaptive 6 mm | 57 551 | 20 113 (34.9 %) | 46.5 % | 0 | 0 | 0.106 / 0.091 | Within |
| operator | 5 | Project Curve 6 mm | 22 675 | 905 (4.0 %) | 17.2 % | 78 | 966 | 0.174 / 0.174 | Within |
| operator | 6 | Pocket 2 mm | 30 364 | 0 (0.0 %) | 3.9 % | 0 | 0 | 0.055 / 0.055 | Within |
| operator | 7 | Pocket 2 mm | 435 | 360 (82.8 %) | 85.2 % | 0 | 0 | 0.054 / 0.055 | Within |
| operator | 8 | Pocket 2 mm | 900 | 900 (100 %) | 99.9 % | 0 | 0 | none / 0.055 | Within, peak 0.0000, population 0 |
| operator | 9 | Project Curve V | 22 629 | 50 (0.2 %) | 15.3 % | 0 | 8 | 0.178 / 0.178 | Within |
| operator | 10 | Trace V60 | 3 492 | 0 | 1.0 % | 0 | 0 | 0.064 / 0.064 | Within |
| operator | 12 | Face 18 mm | 90 004 | 1 | 0.0 % | 0 | 0 | 0.179 / 0.179 | Within |
| operator | 13 | Trace V60 | 1 041 | 0 | 5.3 % | 0 | 0 | 0.064 / 0.064 | Within |
| operator | 14 | 3D Adaptive 18 mm | 90 116 | 41 094 (45.6 %) | 48.6 % | 0 | 0 | 0.135 / 0.139 | Within |
| operator | 15 | 3D Adaptive 6 mm | 67 395 | 11 904 (17.7 %) | 82.1 % | 0 | 0 | 0.111 / 0.101 | Within |
| operator | 17 | Profile 6 mm | 120 546 | 35 (0.0 %) | 0.3 % | 0 | 0 | 0.174 / 0.174 | Within |
| work | 1 | 3D Adaptive 6 mm | 696 121 | 20 059 (2.9 %) | 6.5 % | 0 | 0 | 0.117 / 0.117 | Within |
| work | 2 | 3D Adaptive 6 mm | 28 706 | 2 854 (9.9 %) | 34.0 % | 0 | 0 | 0.100 / 0.094 | Within |
| work | 5 | Project Curve 6 mm | 22 689 | 443 (2.0 %) | 35.0 % | 4 361 | 4 749 | 0.167 / 0.166 | Within |
| work | 6 | Pocket 2 mm | 40 584 | 0 | 3.2 % | 0 | 0 | 0.055 / 0.055 | Within |
| work | 7 | Pocket 2 mm | 432 | 0 | 10.4 % | 0 | 0 | 0.055 / 0.055 | Within |
| work | 8 | Pocket 2 mm | 840 | 0 | 0.0 % | 0 | 0 | 0.055 / 0.055 | Within |
| work | 9 | Project Curve V | 22 633 | 9 | 19.1 % | 38 | 40 | 0.178 / 0.178 | Within |
| work | 10 | Trace V60 | 3 492 | 0 | 1.0 % | 0 | 0 | 0.064 / 0.064 | Within |
| work | 12 | Face 18 mm | 90 004 | 0 | 1.3 % | 0 | 0 | 0.179 / 0.179 | Within |
| work | 13 | Trace V60 | 1 041 | 0 | 9.1 % | 0 | 0 | 0.064 / 0.064 | Within |
| work | 14 | 3D Adaptive 6 mm | 681 336 | 55 441 (8.1 %) | 33.3 % | 0 | 0 | 0.113 / 0.111 | Within |
| work | 17 | Profile 6 mm | 120 511 | 35 | 0.5 % | 18 | 18 | 0.174 / 0.174 | Within |

idx 0 and idx 11 remove no material in the simulation (0 removing samples); idx 3 and idx 4 are
drill cycles.

What the numbers say:

- The filter drops most of the body of an op whose stated feed is above the feed the modulator
  ships. Operator idx 1: the stated feed is 8475 mm/min and holds a chip-thinning lift; the
  modulator holds the peak chip at the band maximum 0.1791 mm, which is 6448 mm/min at a wide
  cut; 6448 is below 0.95 x 8475. The gate then judges 8.6 % of the steady samples.
- Operator idx 8: the gate's population is zero and the verdict is "Within, peak 0.0000". The
  stated chip (0.0699 mm) is above the band maximum (0.0550 mm); the modulator lowers every
  move to about 990 mm/min, below 0.95 x 1258.
- The Pocket slot ring (P23d) is NOT dropped on idx 6: its capped feed is 989.6 mm/min, 98 %
  of the steady feed 1010 (19 251 capped samples in the work file, 15 968 in the operator
  file, all inside the gate).
- Without the filter no verdict on wanaka changes by this readout. The band minimum exists on
  idx 5 and idx 9 only. The gate's low side reads the median, and each median stays above the
  minimum (idx 5: 0.166 to 0.174 against 0.1243). The count of samples under the minimum on
  operator idx 5 rises from 78 to 966.

## 5. The slot and groove ops per sample (item 2, post-simulation part)

Steady samples. Peak force = the peak engaged-edge force; budget 35.32 N. None of these ops
reads the budget today (`cut_force` card: "no force limit on this op family").

| File | idx | Op | Samples | Depth max / p95 (mm) | Peak force max / p95 (N) | Max / budget | Samples over the budget | Mean force max / p95 (N) | Deflection max / p95 (um) | Power max (kW) / available |
|---|---|---|---|---|---|---|---|---|---|---|
| operator | 5 | Project Curve 6 mm | 22 675 | 14.33 / 5.71 | 127.6 / 76.9 | 3.61 | 20 973 (92 %) | 142.9 / 54.9 | 36.6 / 28.1 | 0.476 / 0.663 |
| work | 5 | Project Curve 6 mm | 22 689 | 8.32 / 5.61 | 94.6 / 74.2 | 2.68 | 19 875 (88 %) | 74.7 / 52.7 | 32.8 / 27.3 | 0.563 / 1.5 |
| operator | 9 | Project Curve V 20 | 22 629 | 5.59 / 1.45 | 41.6 / 11.3 | 1.18 | 9 | 29.9 / 8.1 | 67.3 / 32.4 | 0.034 |
| work | 9 | Project Curve V 20 | 22 633 | 2.42 / 1.43 | 18.8 / 11.0 | 0.53 | 0 | 13.6 / 7.9 | 42.0 / 32.1 | 0.011 |
| both | 10 | Trace V60 slit | 3 492 | 4.60 / 4.60 | 36.0 / 36.0 | 1.02 | 1 144 (33 %) | 32.0 / 29.8 | 6.0 / 6.0 | 0.088 to 0.118 |
| both | 13 | Trace V60 engrave | 1 041 | 0.60 / 0.60 | 4.7 / 4.7 | 0.13 | 0 | 3.6 / 3.2 | 1.0 / 1.0 | 0.002 |

- idx 5 is the heavy one. Its real depth is not the 4 mm offset: the tool follows the model
  surface under standing stock, so it reads 5.7 mm at the 95th percentile and up to 14.3 mm
  (operator) or 8.3 mm (work). About 90 % of its samples are over the budget; the worst is
  3.6 x (operator) and 2.7 x (work). The pre-simulation value (51 N at 4 mm) is below the
  simulated 95th percentile (74 to 77 N).
- idx 10 is at the budget (1.02 x). Its real depth is 4.60 mm, not the stated 2.6 mm per pass.
- idx 9 is over only on the operator file, on 9 samples. idx 13 is far under.
- Deflection is under 200 um on all four. The worst is idx 9 (67 um).

## 6. The adaptive roughs after the simulation (item 3, post-simulation part)

Steady samples, peak force against 35.32 N.

| File | idx | Samples | Depth max / p95 (mm) | Peak force max / p95 (N) | Max / budget | Samples over the budget | Depth verdict today |
|---|---|---|---|---|---|---|---|
| operator | 1 | 45 027 | 10.0 / 9.0 | 138.5 / 118.5 | 3.92 | 38 181 (85 %) | Within (10.0 of 36.0 mm) |
| operator | 2 | 57 551 | 20.7 / 6.0 | 84.2 / 37.7 | 2.38 | 3 797 (6.6 %) | Exceeds (20.7 of 12.0 mm) |
| operator | 14 | 90 116 | 5.73 / 3.67 | 62.5 / 50.1 | 1.77 | 27 596 (31 %) | Within (5.73 of 36.0 mm) |
| operator | 15 | 67 395 | 6.41 / 1.90 | 69.7 / 21.7 | 1.97 | 157 (0.2 %) | Within (6.41 of 12.0 mm) |
| work | 1 | 696 121 | 12.0 / 6.0 | 66.0 / 37.0 | 1.87 | 59 809 (8.6 %) | Within (12.0 of 12.0 mm) |
| work | 2 | 28 706 | 16.95 / 2.45 | 46.4 / 25.6 | 1.31 | 92 (0.3 %) | Exceeds (16.95 of 12.0 mm) |
| work | 14 | 681 336 | 3.36 / 3.00 | 35.28 / 27.4 | 1.00 | 0 | Within (3.36 of 12.0 mm) |

- The simulated worst force is above the pre-simulation value on every 6 mm rough: the cut
  takes more than the stated depth in places (work idx 1 reads up to 12.0 mm, twice the stated
  6.0 mm; idx 2 reads 17 to 21 mm). The pre-simulation value describes the stated cut only.
- Work idx 1: 8.6 % of the steady samples are over the budget, and the 95th percentile is
  37.0 N (1.05 x). The stated cut is a small step over the budget, not a factor.
- Operator idx 1: 85 % of the steady samples are over; the 95th percentile is 3.4 x.

## 7. Entry, cleanup, link and capped samples (item 4)

Removing samples that `is_steady_state_for_gate` leaves out. A plunge sample has no arc, so
no power, force or deflection model reads it ("no model"). Max / p95.

### 7.1 Ramp and helix entries

| File | idx | Kind | Samples | Time (s) | Depth (mm) | Peak force (N) | Over the budget | Deflection (um) | Power (kW) |
|---|---|---|---|---|---|---|---|---|---|
| operator | 1 | EntryHelix | 1 863 | 4.1 | 1.97 / 1.94 | 29.6 / 29.3 | 0 | 1.9 / 1.9 | 0.362 / 0.357 |
| operator | 1 | EntryRamp | 5 508 | 10.1 | 1.90 / 1.75 | 27.8 / 25.5 | 0 | 1.8 / 1.7 | 0.323 / 0.223 |
| operator | 2 | EntryRamp | 10 533 | 34.8 | 14.51 / 1.70 | 104.8 / 18.1 | 36 | 28.2 / 7.4 | 0.675 / 0.067 |
| operator | 14 | EntryRamp | 2 885 | 5.8 | 1.99 / 1.40 | 29.2 / 20.0 | 0 | 1.9 / 1.3 | 0.329 / 0.173 |
| operator | 15 | EntryRamp | 7 104 | 21.9 | 4.19 / 1.15 | 42.4 / 13.1 | 3 | 16.2 / 5.5 | 0.126 / 0.046 |
| work | 1 | EntryHelix | 15 031 | 20.4 | 5.56 / 1.39 | 31.6 / 14.6 | 0 | 12.0 / 6.0 | 0.114 / 0.051 |
| work | 1 | EntryRamp | 48 | 0.1 | 1.63 / 1.63 | 19.4 / 19.4 | 0 | 8.0 / 8.0 | 0.073 |
| work | 2 | EntryRamp | 1 339 | 4.0 | 14.50 / 1.82 | 80.2 / 17.3 | 14 | 22.7 / 7.2 | 0.639 / 0.085 |
| work | 14 | EntryRamp | 4 646 | 11.3 | 1.95 / 1.37 | 20.9 / 14.6 | 0 | 8.6 / 6.1 | 0.109 / 0.069 |
| both | 12 | EntryRamp (Face) | 1 189 | 2.0 to 2.7 | 0.50 | 6.2 to 7.5 | 0 | 0.5 | 0.07 |
| both | 17 | EntryRamp (Profile) | 2 597 | 5.3 to 12.1 | 1.25 | 18.4 / 17.6 | 0 | 7.7 / 7.4 | 0.04 to 0.10 |
| both | 8 | EntryRamp (Pocket) | 310 | 3.5 to 4.6 | 0.50 | 3.5 to 3.8 | 0 | 6.3 | 0.003 |

- The entries of the bulk roughs (idx 1, 14) are light: under 32 N, under the budget.
- The entries of the rest rough idx 2 are the heavy ones: the worst ramp sample is 104.8 N
  (operator) and 80.2 N (work), at 14.5 mm of engaged depth. They are rare (36 and 14 samples
  over the budget); the 95th percentile is 18 N. idx 2 is the op with the entry-load critical.
- No entry sample is over the power limit or the 200 um deflection limit.

### 7.2 Plunge-type entries (no model)

| File | idx | Kind | Samples | Time (s) | Feed (mm/min) |
|---|---|---|---|---|---|
| both | 5 | EntryPlunge (Project Curve 6 mm) | 2 546 to 2 683 | 15.5 to 30.3 | 1347 (operator), 2500 (work) |
| both | 9 | EntryPlunge (Project Curve V) | 3 356 to 3 450 | 135 to 139 | 381 |
| both | 6 | EntryPlunge (Pocket 2 mm) | 1 238 to 1 329 | 70 to 74 | 240 |
| both | 10, 13 | EntryPlunge (Trace) | 36, 58 | under 1.1 | 762 (operator), 1016 (work) |
| both | 17 | EntryPlunge (Profile; 273 more with no span after a tab) | 51 + 273 | 0.3 to 2.5 | 1582 (operator), 2500 (work) |
| 3D roughs | 1, 2, 14, 15 | EntryApproach, and `Linking` moves inside an entry span | 58 to 596 each | under 4 | 2285 to 2500 |

Each plunge sample removes 0.02 mm of height per sample (`plunge_descent_mm`). The load of
these samples cannot be measured without an axial or inclined engagement model (P29-8).

### 7.3 The waterline cleanup and the link bridges

| File | idx | Kind | Samples | Time (s) | Depth (mm) | Peak force (N) | Over the budget | Deflection (um) | Power (kW) |
|---|---|---|---|---|---|---|---|---|---|
| operator | 1 | WaterlineCleanup | 160 | 0.4 | 3.50 / 3.20 | 32.1 / 29.7 | 0 | 2.0 | 0.08 |
| operator | 2 | WaterlineCleanup | 35 | 0.1 | 3.0 | 35.3 / 32.5 | 0 | 14.0 | 0.14 |
| operator | 14 | WaterlineCleanup | 3 088 | 6.3 | 4.17 / 4.17 | 62.5 / 60.9 | 1 212 | 3.9 | 0.66 / 0.35 |
| work | 1 | WaterlineCleanup | 4 604 | 18.0 | 13.59 / 6.0 | 90.3 / 37.0 | 286 | 24.9 / 13.5 | 0.66 / 0.14 |
| work | 2 | WaterlineCleanup | 35 | 0.1 | 3.0 | 31.0 / 28.6 | 0 | 12.3 | 0.17 |
| work | 14 | WaterlineCleanup | 7 227 | 14.8 | 3.0 / 2.5 | 31.3 / 28.5 | 0 | 12.5 / 11.5 | 0.12 |
| operator | 1 | LinkBridge | 26 | 0.1 | 10.0 | 136.8 / 132.1 | 6 | 7.3 | 0.85 |
| operator | 14 | LinkBridge | 524 | 0.9 | 3.67 | 53.0 / 47.6 | 235 | 3.3 | 0.25 |
| work | 1 | LinkBridge | 495 | 0.9 | 6.0 | 51.8 / 43.6 | 39 | 18.8 | 0.16 |
| operator | 2, work 2 | LinkBridge | 140 | 0.3 to 0.4 | 6.0, 2.45 | 50.5, 27.1 | 2, 0 | 18.3, 10.9 | 0.11, 0.07 |
| operator | 15, work 14 | LinkBridge | 904, 582 | 2.1, 1.3 | 0.50, 3.0 | 5.8, 32.2 | 0 | 2.5, 12.8 | 0.02, 0.18 |

- Work idx 1: the cleanup holds the worst force of the whole op, 90.3 N at 13.6 mm of depth.
  The steady body's worst is 66.0 N. No gate reads the cleanup sample.
- Operator idx 14: 39 % of the cleanup samples are over the budget, at the body's worst force.
- The link-bridge values read neighbouring stock by the gate's own rule (P28 row 23); they
  are listed for size only.

### 7.4 Moves with `Move::feed_cap`, by `FeedCapSource`

These samples are mostly inside the steady population already. Max / p95 of the peak force.

| Source | operator idx 1 | operator idx 2 | operator idx 14 | operator idx 15 | work idx 1 | work idx 2 | work idx 14 |
|---|---|---|---|---|---|---|---|
| `planned_turn`: samples | 733 | 1 585 | 1 363 | 1 956 | 2 114 | 607 | 4 714 |
| force (N) | 118.1 / 96.2 | 75.8 / 38.4 | 55.1 / 35.0 | 31.6 / 9.3 | 49.7 / 42.3 | 46.4 / 18.8 | 34.2 / 21.7 |
| over the budget | 384 | 98 | 65 | 0 | 565 | 1 | 0 |
| `capped_2d`: samples | 31 | 1 335 | 189 | 2 756 | 1 007 | 557 | 2 661 |
| force (N) | 136.1 / 123.1 | 67.7 / 46.0 | 38.6 / 26.5 | 29.0 / 9.4 | 50.4 / 43.9 | 35.9 / 22.7 | 32.1 / 9.0 |
| over the budget | 18 | 149 | 5 | 0 | 481 | 1 | 0 |
| `capped_split_3d`: samples | 25 | 490 | 238 | 20 | 535 | 8 | 1 636 |
| force (N) | 126.4 / 117.9 | 39.0 / 34.2 | 52.0 / 43.0 | 24.8 / 23.8 | 43.7 / 29.0 | 16.3 | 35.3 / 26.6 |
| over the budget | 19 | 10 | 29 | 0 | 21 | 0 | 0 |
| `stock_read_3d`: samples | 132 | 696 | 1 817 | 13 | 5 476 | 18 | 9 640 |
| force (N) | 138.5 / 132.5 | 84.2 / 50.0 | 62.5 / 54.0 | 19.6 | 66.0 / 40.9 | 41.8 | 35.3 / 28.5 |
| over the budget | 100 | 265 | 564 | 0 | 2 961 | 2 | 0 |
| `departure_window`: samples | 0 | 1 277 | 1 679 | 2 859 | 0 | 499 | 2 080 |
| force (N) | - | 54.2 / 35.7 | 46.2 / 34.4 | 23.6 / 12.6 | - | 40.5 / 32.5 | 34.2 / 19.7 |
| over the budget | - | 67 | 74 | 0 | - | 13 | 0 |
| Pocket slot ring (idx 6, no `FeedCap` item): samples / force | 15 968 / 23.4 / 5.1 (operator) | | | | 19 251 / 5.1 / 5.1 (work) | | |

- The capped steps (`stock_read_3d`, `capped_2d`) hold the worst steady force of each op:
  operator idx 1 138.5 N, idx 2 84.2 N, idx 14 62.5 N; work idx 1 66.0 N. The cap lowers the
  feed; it does not bring the force under the budget. On work idx 1, 54 % of the
  `stock_read_3d` samples are over the budget.
- No capped sample is over the power limit or the 200 um deflection limit.
- The Pocket slot ring is far under the budget on this job (a 2 mm tool).

## 8. M5: the depth gate on every sample

The depth gate's population: `is_cutting`, `axial_engagement_mm > 0`, steady. Today the gate
judges the one sample with the largest depth.

| Rule today | Ops | Samples over the limit when every sample is read | Verdict today |
|---|---|---|---|
| Force budget (Pocket, Face, Profile) | idx 6, 7, 8, 12, 17, both files | 0 of 60 381, 616, 990, 90 049, 121 634 | Within or Reported |
| Factor 2 x D (Adaptive) | idx 1, 14, 15 operator; idx 1, 14 work | 0 over the Factor cap | Within |
| Factor 2 x D (Adaptive) | idx 2 | 63 (operator), 4 (work) over the Factor cap | Exceeds (one sample) |
| None (Trace, Finish) | idx 5, 9, 10, 13 | no limit | Reported, no bound |

The same population read against the force budget (the P29-4 and P29-5 rule):

| File | idx | Samples over the budget at the gate's cut | Of |
|---|---|---|---|
| operator | 1 | 38 181 | 84 417 |
| operator | 2 | 3 797 | 105 318 |
| operator | 5 | 20 973 | 27 174 |
| operator | 9 | 9 | 22 696 |
| operator | 10 | 1 144 | 3 492 |
| operator | 14 | 27 596 | 168 611 |
| operator | 15 | 157 | 128 953 |
| work | 1 | 59 809 | 876 293 |
| work | 2 | 92 | 53 548 |
| work | 5 | 19 875 | 27 185 |
| work | 10 | 1 144 | 3 492 |
| work | 14 | 0 | 961 681 |

M5 as a change by itself moves nothing on wanaka: every op that reads the force budget today
has zero samples over it. The counts above come from the SCOPE change (P29-4, P29-5), not from
"every sample in place of the peak sample".

## 9. What section 5 of `P29_FINDINGS.md` gets wrong

1. **"Chip gate without the 95 % filter: slot rings of idx 6, 7, 8, 11; capped steps of the 3D
   roughs."** The slot ring is not dropped (its feed is 98 % of the steady feed on idx 6). The
   filter drops the BODY of an op whose stated feed is above the modulated feed: 91 % of
   operator idx 1, 83 % and 100 % of operator idx 7 and 8, 35 to 46 % of operator idx 2 and
   14. The cause is the modulation, not the cap. On the work file the filter drops 3 to 10 %
   of the roughs.
2. **"Risk: slow moves read as under the band."** Not on wanaka. A band minimum exists on
   idx 5 and idx 9 only, and no median falls under it. The real defect is the other way: the
   chip gate of operator idx 8 has no population and prints "Within".
3. **"Slot and groove ops ... size not measured / Unknown".** Measured. idx 5 is 2.7 to 3.6 x
   over the budget on about 90 % of its samples, the largest relative overload after operator
   idx 1. idx 10 is at the budget. idx 9 and idx 13 are under (idx 9: 9 samples over on the
   operator file). The pre-simulation value of idx 5 at the 4 mm offset (51 N) understates the
   simulated cut (95th percentile 74 to 77 N), because the tool cuts deeper than the offset.
4. **"Force budget on Adaptive: idx 14 (3 mm) no change" (work).** Confirmed before the
   simulation (0.83 x). After it the worst sample is at the budget (35.28 of 35.32 N) and zero
   samples are over. On the operator file idx 14 is 1.63 x over before the simulation and 31 %
   of its samples are over after it; section 5 says only "over".
5. **"6 mm idx 1: depth 6.0 to about 3.85 mm".** The size is right (3.95 mm at the stated
   width, 3.75 mm at the swept width). What section 5 does not say: the stated cut is 1.12 x
   the budget, and after the simulation only 8.6 % of the samples are over, but the worst is
   1.87 x because the cut reaches 12 mm in places.
6. **"Force and deflection read the slot ring: Pockets idx 6, 7, 8, 11: the depth can fall."**
   Not on this job. The slot-ring samples of idx 6 peak at 5.1 N (work) and 23.4 N (operator).
   idx 11 removes no material in the simulation.
7. **"Depth gate on every sample: more exceeds rows on wide shallow samples. Low to medium."**
   Zero on wanaka for the ops that have a force budget today. idx 2 goes from 1 sample to 63
   (operator) or 4 (work) under the Factor rule; the verdict is already "Exceeds".
8. **"Gates judge entries and cleanup ... idx 1 helix entries".** The helix and ramp entries
   of the bulk roughs are under the budget (under 32 N). The heavy entries are the ramps of
   the rest rough idx 2 (80 to 105 N worst, rare). The cleanup matters more than section 5
   says: on work idx 1 it holds the worst force of the op (90.3 N).
9. Not in section 5: the capped steps hold the worst steady force of each 3D rough. The cap
   lowers the feed and leaves the force 1.8 to 3.9 x over the budget.

## 10. Not measured

- Modulation off. The rule was "only if each run with modulation on took under 5 minutes";
  the work run took 8 min 15 s.
- The scallop finish (idx 16), by instruction. Work idx 15 (disabled).
- idx 0 (Face back) and idx 11 (clamp pads): the simulation removes no material there (0
  removing samples; the chip verdict is `AllSamplesAirCutOrRapid`). Not followed.
- The load of plunge samples (sections 7.2): no arc, so no model function gives a value.
  Product code is necessary (an axial or inclined engagement).
- The chip verdict "with and without the filter" as a gate verdict. The probe counts samples
  under the band; it does not run `chipload::evaluate` without the filter (the function is
  `pub(crate)` and the filter is inside it). A product change is necessary for the exact
  verdict.
- FM1. No FM1 cell was measured.
- The drill gates of idx 3 and idx 4 (`DrillPlungeFeed` reads 166.7 against a bound of 50 and
  prints `Within`): a side finding, not followed.

## 11. Side findings (not followed)

- The trace holds the feed before the modulation for an op that no later simulation step ran
  again, and the feed after it for an op that one did. So the chip gate's filter population
  depends on how many simulation passes ran (operator idx 15: 17.7 % dropped by the sample
  feed, 82.1 % by the effective feed).
- 3D rough moves tagged `Linking` and `EntryApproach` sit inside `Entry` spans and remove
  material by the plunge axis (95 to 596 samples per op).
- Profile: 273 `EntryPlunge` samples after tabs are in no span ("in_transit, no span"); the
  gates drop them as phantom, not as entries.
- The engaged depth of the 6 mm rest rough idx 2 reaches 17 to 21 mm, above the tool's 2 x D.
- Three ops show the same worst force 35.28 N (a full slot, 3 mm deep, at the band chip
  0.127 mm with the 6 mm tool). It is 0.04 N under the budget by coincidence of the numbers.
