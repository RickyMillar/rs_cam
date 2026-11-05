# The models that make an operator number

Hand-off commit `f0150aeb5`. Each model gives the formula, the inputs, the source, the constants
and the known limits.

A constant has one of three states:

- **F201 Cited**: a row of the registry with a printed source.
- **F201 Unsourced**: a row of the registry with no printed source. The registry is
  `formula_constants()` at `crates/rs_cam_core/src/feeds/mod.rs:2167`. The citation note is
  `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md`. The sentry
  `every_feeds_formula_constant_states_its_source_f201` pins the count: 161 Unsourced
  (`crates/rs_cam_core/tests/every_feeds_formula_constant_states_its_source_f201.rs:91`).
- **Not in the registry**: the line says where the value comes from (a `CREDITS.md` row, a doc
  comment, or none).

Line numbers in the F201 note point at old code lines. Use the row name to find the code.

The explainer page https://claude.ai/artifact/78xXh5LQ2oPoPsErTZZnQy (source
`planning/feeds_contract_2026-11-03/explainer_p13c/`, claims checked by
`planning/feeds_contract_2026-11-03/explainer_p13c/assert_claims.py`) shows the chain at one point.

## M1. Suggest: the calculator chain

The driver is `calculate_with` in `crates/rs_cam_core/src/feeds/calculator.rs`. One function per
step (`crates/rs_cam_core/src/feeds/CLAUDE.md:6`).

| Step | Formula | Code |
|---|---|---|
| 1 rpm | `rpm = Vc x 1000 / (pi x d_eng)`, clamped to the machine rpm range | `crates/rs_cam_core/src/feeds/calculator/speed.rs:84-110` |
| 2 chip | the vendor row chip (band middle), scaled by `(D / D_row)^0.61` off the printed size; else the formula chip `k0 x d^p x (1 / feed_scale)^q` | `crates/rs_cam_core/src/feeds/calculator/chip.rs:15`, `crates/rs_cam_core/src/feeds/calculator/chip.rs:36` |
| 3-4c cut | engagement floor, flute guard, slot cap, shank | `crates/rs_cam_core/src/feeds/calculator/cut.rs:14`, `crates/rs_cam_core/src/feeds/calculator/cut.rs:73`, `crates/rs_cam_core/src/feeds/calculator/cut.rs:141` |
| 5 feed | `F = chip x lift x rpm x Z` (lift: M2) | `crates/rs_cam_core/src/feeds/calculator/feed.rs:14-40`, `crates/rs_cam_core/src/feeds/calculator/feed.rs:62` |
| 5b long tool | a long-tool share; a load target, not a feed factor | `crates/rs_cam_core/src/feeds/calculator/feed.rs:143` |
| 6 power | the power ladder against the spindle curve (M6) | `crates/rs_cam_core/src/feeds/calculator/power.rs:59` |
| 7 feed cap | at the cap: arm L (lower rpm, same chip) and arm S (same rpm, smaller chip); power picks (Q1) | `crates/rs_cam_core/src/feeds/calculator/feed.rs:190` |
| 8 plunge | family fraction x shipped feed (`PLUNGE_RULES`; flat end: 1/Z) | `crates/rs_cam_core/src/feeds/calculator/close.rs:63`, `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs:254` |
| 9b rubbing floor | warns below the floor, never lifts | `crates/rs_cam_core/src/feeds/calculator/close.rs:108` |
| 9c drill | drill rules read the (Pocket, Roughing) side row x 1/Z | `crates/rs_cam_core/src/feeds/calculator/close.rs:170`, `crates/rs_cam_core/src/feeds/extrapolation/drill.rs:86` |
| ramp | `floor(min(feed, chip x rpm x Z / tan theta))` (G6) | `crates/rs_cam_core/src/feeds/ramp.rs:383`; rule `crates/rs_cam_core/src/feeds/CLAUDE.md:21` |

Inputs: tool (D, Z, flute length, stickout, shape: K3 facts), material, machine (rpm range, power,
feed cap, rigidity), the op's `CutSpec` (family, pass role, stated ap and ae).

Sources: vendor rows `crates/rs_cam_core/data/vendor_lut/observations/` and their manifest
(`CREDITS.md:247-261`); Garr (`CREDITS.md:363-364`); the depth table (`CREDITS.md:369-375`);
plunge claims (`CREDITS.md:906`).

| Constant | Value | State | Code |
|---|---|---|---|
| Cutting speed, solid wood | 200 m/min | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:52`) | `crates/rs_cam_core/src/material/mod.rs:736` |
| Formula chip `k0`, `p`, `q` | 0.024, 0.61, 1.26 | F201 Unsourced, "Shapeoko empirical data" (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:229`) | `crates/rs_cam_core/src/machine/mod.rs:47-55` |
| Size exponent `CHIPLOAD_DIAMETER_EXPONENT` | 0.61 | Not in the registry; a fitted summary of seven vendors (its doc says never cite it as physical) | `crates/rs_cam_core/src/feeds/vendor_lookup.rs:526-541` |
| Long-tool thresholds and shares | 6 / 4 x D; 0.75 / 0.88 | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:127-128`) | `crates/rs_cam_core/src/feeds/mod.rs:1994-2000` |
| Flute guard | 0.8 | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:129`) | `crates/rs_cam_core/src/feeds/mod.rs:1848` |
| Slot threshold | 0.85 D | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:131`) | `crates/rs_cam_core/src/feeds/mod.rs:1919-1923` |
| Rubbing floor | 0.025 mm/tooth | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:126`) | `crates/rs_cam_core/src/feeds/mod.rs:1402` |
| Engagement floor `MIN_AP_MM` | 0.05 mm | F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:219`) | `crates/rs_cam_core/src/feeds/mod.rs:1974` |
| Default stepover table `ae_factor`, `ap_factor` | per family | F201 Unsourced (Pocket roughing `ae`: F201 Cited, the corner rule, `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:256`) | `crates/rs_cam_core/src/feeds/mod.rs:2041` |
| Drill plunge ceilings, wood / plywood / sheet | 580 / 580 / 720 mm/min per mm | F201 Cited, Amana Spektra v24 ramp-down rows (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:36-38`) | `crates/rs_cam_core/src/material/mod.rs:645` |
| Flat-end plunge fraction | 1/Z | Not in the registry; a G10 claim with named witnesses (Amana, Sienci, IDC, Carbide 3D) | `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs:254-272` |

Known limits: the 20 ball and 8 V-bit formula cells (WHAT_IS_DONE R40); the size law (R39); the
long-tool share moves no feed (R2); the Freud rows print no rpm, so Step 1 gives it
(`planning/feeds_contract_2026-11-03/phases/P13.md:422-426`).

## M2. Radial chip thinning

`lift = 1 / sin psi` for `psi < 90 deg`, else 1, with `cos psi = 1 - ae / r` and `r = d_eng / 2`.
A finish pass and a drill cycle take lift 1 (decision L2).

- Code: `crates/rs_cam_core/src/feeds/geometry.rs:30-37` (lift), `crates/rs_cam_core/src/feeds/force.rs:27`
  (immersion), `crates/rs_cam_core/src/feeds/geometry.rs:47-91` (`RadialThinning`).
- Source: Garr, `CREDITS.md:384-391`.
- Constants: none.
- Readers: Suggest, the post-sim gate, the modulator, `dressup::feedopt` (`crates/rs_cam_core/src/feeds/CLAUDE.md:14`).
- Limits: no cap (R42); Waterline thins (R41); the Harvey finish witness (R38); the bull-nose
  `d_eng` defect gives a large false lift below the corner radius (R30).

## M3. The vendor depth table (a limit, not a chip scale)

Allowed chip at depth `ap`: `band max x s(ap / D)`, with `s` = 1.0 at 1 x D, 0.75 at 2 x D,
0.5 at 3 x D, linear between, 0.5 above. The limit is the inverse: the deepest `ap / D` where the
allowed chip is at or above the peak chip.

- Code: `crates/rs_cam_core/src/feeds/geometry.rs:215-226` (table), `crates/rs_cam_core/src/feeds/geometry.rs:352-363`
  (inverse, `LimitKind::VendorDepthTable`).
- Source: Onsrud, Freud, Amana print the three points (`CREDITS.md:369-375`). Not in the registry.
  The three points are Cited by `CREDITS.md`; the interpolation is the crate's own.
- Limits: the vendors print no width; the ceiling divides by the nominal diameter; a point past it
  warns, it does not cap (R37).

## M4. The force line (one per material)

`Ks = Ks_n x rho`, `F_edge = Int_n x rho`. `Ks_n` and `Int_n` are the upper envelope over grain
angle and mode of the Curti 2021 Table 5 helix-0 quadratics. MDF reads Goli 2018 directly.
`rho = SG x 1120` (FPL Table 5-3a, or 5-5a by Eq. 4-11).

- Code: `crates/rs_cam_core/src/material/force_line.rs:533-550` (density law),
  `crates/rs_cam_core/src/material/force_line.rs:257-273` (envelope), `crates/rs_cam_core/src/material/mod.rs:1307` (`force_line`).
- Source: `CREDITS.md:450-475`.
- Constants (all not in the registry; Cited by `CREDITS.md` and the doc comment beside each):
  the four Curti quadratics `crates/rs_cam_core/src/material/force_line.rs:189-202`; the density
  range 287-1080 kg/m3 `crates/rs_cam_core/src/material/force_line.rs:74`; the chip range
  0.04-0.10 mm `crates/rs_cam_core/src/material/force_line.rs:78`; Goli `Ks` 31.44 and `Int` 3.36
  `crates/rs_cam_core/src/material/force_line.rs:90-94`.
- Limits: plywood, HDF, particleboard and non-wood materials have no line and refuse
  (`MaterialUnvalidated`); a chip outside the printed range only names its regime; the grain
  factor is 1.0; `K_r = 0` (R44).

## M5. The engaged-edge force (K7)

The edge is split into helical elements at radius `d_eng / 2`. At each rotation sample the force
vector is `Ks fz A + F_edge B` (shear and edge sums).

- Mean over one revolution: `ap Z / (2 pi) x (Ks fz (1 - cos psi) + F_edge psi)`. Power reads it.
- Peak resultant over the rotation samples. Deflection and the force budget read it.
- Feed cap at a force budget: the smallest positive root of `|F(fz)| = budget` over the samples.
- Code: `crates/rs_cam_core/src/feeds/engagement.rs:230-255`, `crates/rs_cam_core/src/feeds/engagement.rs:275`,
  `crates/rs_cam_core/src/feeds/engagement.rs:323`.
- Source: `CREDITS.md:484`; the force line (M4).
- Constants: helix 30 deg default, F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:221`,
  `crates/rs_cam_core/src/tool/mod.rs:219`); `K_r = 0`, no source (`crates/rs_cam_core/src/feeds/engagement.rs:220-222`).
- Limits: one radius for the whole edge (R43); two diameter rules (R30); mean equals power only
  on a flat end mill (`planning/feeds_contract_2026-11-03/phases/P11.md:302-304`).

## M6. Spindle power

`P(F) = grain x (Ks x A_section x F / 60e6 + F_edge x ap x Vc x (Z psi / 2 pi) / 60e6)` kW.
Available power on a VFD: `P_rated x min(rpm, rpm_rated) / rpm_rated`. Pre-sim power is the
maximum over the `PowerCases` of the `CutSpec`.

- Code: `crates/rs_cam_core/src/tool_load/power.rs:88-130`, `crates/rs_cam_core/src/machine/mod.rs:1076-1088`,
  `crates/rs_cam_core/src/feeds/operating_point.rs:219`.
- Source: the force line (M4). The available-power line is the definition of the named
  `VfdConstantTorque` model, not a fitted constant. No measurement in the repo shows that the
  operator's spindle follows it; the VFD power ladder (brief part B) tests it.
- Constants: the rated power and rpm are machine data (rivmap100: `planning/fixtures/rivmap100/rivmap100.toml:38-40`).
  Not in the registry.
- Limits: no spindle or VFD efficiency term; the model is not measured on a spindle (brief
  part C, the VFD power ladder).

## M7. Tool deflection

Cantilever bending (Euler-Bernoulli), integrated over the shank and the flute section (an
equivalent bending diameter), load at `stickout - ap / 2`. Compliance = deflection per newton;
deflection = compliance x peak edge force (M5). The envelope `Deflection` edge is the depth where
deflection meets the target.

- Code: `crates/rs_cam_core/src/tool/mod.rs:755-800`, `crates/rs_cam_core/src/tool_load/deflection.rs:80-90`,
  `crates/rs_cam_core/src/feeds/predict.rs:615`, `crates/rs_cam_core/src/feeds/cutter_constraints.rs:101`.
- Source: `CREDITS.md:484`.
- Constants: target 200 um, F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:176`,
  `crates/rs_cam_core/src/feeds/suggest/invariants.rs:53`); carbide E 600 GPa, not in the registry,
  a handbook range in the doc comment and no file in the repo (`crates/rs_cam_core/src/compute/tool_config.rs:128-130`).
- Limits: bending only; no collet, spindle or frame compliance (O2); 16-step scan (R46).

## M8. Machine rigidity: the force budget and the caps

- Hobby budget: the largest peak engaged-edge force (M5) of six printed hobby full-slot cuts
  (Sienci, Carbide 3D). Code `crates/rs_cam_core/src/feeds/force_budget.rs:149-261`. Value 35.32 N,
  F201 Cited (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:254`). Source `CREDITS.md:937-962`.
- Roughing depth (Pocket, Profile): the shallowest depth where the peak force at the cut width
  and chip meets the budget. Code `crates/rs_cam_core/src/feeds/force_budget.rs:351`,
  `crates/rs_cam_core/src/feeds/force_budget.rs:497`.
- Corner coverage width: `(1 + 1/sqrt 2) / 2 x D` (0.854 D), derived. F201 Cited
  (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:255`). Code `crates/rs_cam_core/src/feeds/force_budget.rs:427-429`.
- Slot cap `slot_doc_factor` 0.25 D, Adaptive depth `adaptive_doc_factor` 2.0 D, Adaptive width
  `adaptive_woc_factor` 0.25 D: machine data, the Shapeoko port, named hobby grade. Not in the
  registry (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:240-241`); no source.
  Code `crates/rs_cam_core/src/machine/mod.rs:331-339`.
- An industrial machine has no default; its table must state the block
  (`crates/rs_cam_core/src/machine/CLAUDE.md:31`).
- Limits: the budget stands in for frame stiffness, with no measured stiffness (O2); the MDF row
  sets it (R36); the peak force plateaus with depth, so the solved depth jumps
  (`planning/feeds_contract_2026-11-03/phases/P13.md:529-532`); the scans (R46); the saved copy (R54).

## M9. The safe envelope and the pick (K2b)

The envelope is the (ap, ae) plane with one `Limit` per `LimitKind`: flute guard, cutting length,
vendor axial, vendor depth table, rigidity, corner coverage, slot, power, deflection, chip band,
rubbing floor, feed cap, diameter, move budget, engagement floor. The proposal is the point with
the most removal per minute inside every limit, never above a stated value.

- Code: `crates/rs_cam_core/src/feeds/envelope.rs:134-174` (kinds), `crates/rs_cam_core/src/feeds/envelope.rs:804`
  (`for_cut`), `crates/rs_cam_core/src/feeds/envelope.rs:1131` (`pick`), `crates/rs_cam_core/src/feeds/envelope.rs:2424`
  (rigidity edge and its refine).
- Constants: the limits above; numerical steps only (refine 12 steps, 4 split levels).
- Limits: power is read between width samples (R56); cost (R57).

## M10. The K5 physical rules

Four rows: a non-centre-cutting plunge, reach past the flute, reach to the holder, the V-tip
cone height. Each warns on every surface (Q6).

- Code: `crates/rs_cam_core/src/compute/feasibility.rs:164-210`.
- Constants: the V-tip tolerance is half the last printed digit (`crates/rs_cam_core/src/compute/feasibility.rs:212-216`).
- Limits: warnings only; the rules read stated facts (D12).

## M11. The simulation measures (the post-sim gate)

- Population: steady-state samples only, `crates/rs_cam_core/src/tool_load/locality.rs:156`.
- Peak chip per tooth: advance / lift at the measured arc, against the printed band at every
  depth, `crates/rs_cam_core/src/tool_load/display.rs:83`.
- Deflection: peak edge force of the sample x compliance, bound 200 um,
  `crates/rs_cam_core/src/tool_load/deflection.rs:63`.
- Power: `crates/rs_cam_core/src/tool_load/power.rs:219`.
- Depth: the force-budget cap at the peak sample (`the_depth_that_cut_is_a_measured_load_g_s3depth`).
- Load fraction `L`: `crates/rs_cam_core/src/tool_load/load_fraction.rs:36`.
- Placement on the envelope (not fed back, Q5): `crates/rs_cam_core/src/compute/simulate.rs:967`.
- Entry: `evaluate_toolpath`, `crates/rs_cam_core/src/tool_load/mod.rs:611`.
- Limits: two depth fields (R48); the heat map paints the advance (R51); no noise floor (R64);
  18 of 35 FM1 sim cells outside their own envelope (R61).

## M12. Add-door geometry defaults (D8)

A new op has no geometry default without a tool. Each default is a fraction of the tool, and
`geometry_default::write_rule_values` is the one writer (`crates/rs_cam_core/CONTRACTS.md:10`).

- Radial (roughing Pocket family, Profile): the envelope proposal (M9); with no force line or no
  backed row, the corner-coverage width and no depth (`planning/feeds_contract_2026-11-03/phases/P13.md:347-354`).
- Axial: `k x D` rows `AXIAL_K_*`, F201 Unsourced (`planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:203-207`,
  `crates/rs_cam_core/src/compute/geometry_default.rs:685-699`).
- Finish stepover and Z step on a ball or tapered ball: the scallop chord
  `2 sqrt(2 R h - h^2)` (`CREDITS.md:199-205`).
- Sentries: `every_geometry_default_is_a_fraction_of_the_tool_d8`, `the_add_door_stepover_is_the_proposal_p13c`.
- Limits: the axial `k` came from the old 6 mm values (R59); the 0.85 D default meets the slot
  threshold (R53).

## Research behind the limits

- Stepover load: `planning/feeds_contract_2026-11-03/research_ae/REPORT.md`.
- Force model validation (verdict FAIL) and the after page:
  `planning/feeds_contract_2026-11-03/research_p11_step0/REPORT.md`, `planning/feeds_contract_2026-11-03/research_p11_step0/after/`.
- Derate audit: `planning/feeds_contract_2026-11-03/research_derate/REPORT.md`.
- Inputs and assumptions per output: `planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md`.
- Vendor-row chip law: `planning/feeds_contract_2026-11-03/research_p13_items45/FINDINGS.md`.
