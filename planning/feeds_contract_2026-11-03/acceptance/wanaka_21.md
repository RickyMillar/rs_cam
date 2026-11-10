# Wanaka 21: feeds and speeds acceptance table

This table states what each feeds-and-speeds number of the wanaka 21 project
SHOULD be. Each expected value comes from a printed source line in the repo,
the project file, or arithmetic shown in the row. The app is the thing under
test. No expected value comes from the engine code or from the app output.

- Project: `planning/rivmap_block_job/nz_wanaka_21_operator.toml`, 18 toolpaths, white oak.
- Graded build: `0c67ad7fe` (P23a, P23b; commit time 1794276928, 2026-11-10 15:15). The
  running `rs_cam_gui` binary (`target/release`) has mtime 1794277254 (15:20), later than
  HEAD; the process started 2026-11-10 15:38; no edits under `crates/`. Graded by a fresh
  agent on 2026-11-10: PASS 95, FAIL 3, NEEDS-RULING 0, N/A 10. The previous grades: build
  `dce570108` (P21c) after the six rulings PASS 92, FAIL 6, NEEDS-RULING 0; the same build
  before them PASS 64, FAIL 6, NEEDS-RULING 28; build `b9e67b24e` (P20e, 2026-11-09) PASS 47,
  FAIL 23, NEEDS-RULING 28; build `998ac93a3` PASS 31, FAIL 12, NEEDS-RULING 55. N/A is 10
  in each grade from `4b7aa0e96` on.
- App values: MCP `get_suggest_rationale` (Suggest, all 18) and `get_toolpath_params`
  (idx 1 and 14 re-read; the project file did not change since the read at `dce570108`),
  on 2026-11-10. Nobody applied Suggest, so each stored param still holds its old value
  (18000 rpm; 10610 rpm on idx 5 and 17). The Suggest column is the graded value. For idx 5
  and 9 (`shipped_power` unmodeled) the feed, rpm and plunge come from `basis.rpm` (P23b).
- Script: `derive_wanaka_21.py` computes every number and every grade. The
  file `wanaka_21.toml` is its machine-readable output. The script asserts
  the force-line and chip identities that this page states.

## Rules

Status classes:

- **SOURCED**: a printed line fixes the value.
- **DERIVED**: arithmetic from a sourced quantity and physics.
- **POLICY**: a free choice. The operator must rule. The row gives each candidate.
- **UNSOURCED**: no source fixes the value. The row says what is necessary. A bracket from the nearest printed rows is given where one exists.
- **N/A**: the quantity does not apply (for example, a ramp feed on a straight-plunge entry).

Grades (tolerance 1 % relative; the app rounds feeds to whole mm/min, which is far inside 1 %):

- **PASS**: the app value matches the fixed expected value, or it matches every policy candidate.
- **NEEDS-RULING**: the app value matches some candidates only, or it lies inside an UNSOURCED bracket. The `matches` list names the candidates.
- **FAIL**: the app value matches no candidate, or the app prints a number where no model applies.

The rpm row alone carries question P1. The feed, plunge and ramp rows are
graded at the app's own rpm: the script computes each candidate again at that
rpm. The P1a and P1b values stay in the "Expected" text as information. Thus
a feed row tests the chip, the lift and the arithmetic, not the RPM choice.
For a drill, the plunge row repeats the feed row (a drill cycle has one feed).

The "App Suggest" column is the graded value. The "App shipped" column is the
toolpath parameter, which can be an operator value (for example idx 1 feed
8475, idx 3 and 4 drill feed 1000). Shipped values are information only.

## Common inputs

**Machine** (project file lines 60-66 and 85-89; the spindle policy is on lines 50-51): spindle 6000-24000 rpm;
`VfdConstantTorque` 1.5 kW at 24000 rpm; XY rate 15000, Z rate 2500 mm/min.
Available power below 24000 rpm is `1.5 x n / 24000` kW. At 18000 rpm it is 1.125 kW.

**Material caveat** (one statement for all rows): the Amana Spektra chart
prints one "Wood/Plywood" column. It does not split hardwood from softwood.
A white-oak value from it is SOURCED by the cell. The cell is not specific to
hardwood. The Onsrud chart (`onsrud_hard_wood_cutting_data.txt`) is specific
to hardwood. It prints no RPM for the series used here.

**Depth condition**: every chart prints "1 x D" (Spektra line 3, AMS159 line 3,
SpeTool line 14). Every toolpath here has ap/D <= 1 (the rows state ap/D),
so no depth derate applies (Spektra lines 37-39: "2 x D Reduce feed rate by 25%").

**Radial chip thinning** (geometry): for r = ae/D < 1/2 the peak chip is
`fz x 2 sqrt(r(1-r))`. To hold the chart chip as the peak chip, the feed per
tooth is `chip / (2 sqrt(r(1-r)))`. For r >= 1/2 (a slot) the lift is 1. A
ramp cuts a full slot, so the ramp feed has lift 1.

**Force line** (power rows):

- Curti 2021 Table 5, down-milling, helix 0 (`marcon2021_generalized_force_model.txt` line 785):
  `Ks_n = -3e-6 GA^2 + 5e-4 GA + 56e-3`. The vertex is at GA = 5e-4 / 6e-6 = 83.33 deg:
  Ks_n = 0.056 + (5e-4)^2 / (4 x 3e-6) = 0.0768333 N/mm^2 per kg/m^3.
- Line 787: `int_n = -3e-7 GA^2 + 4e-5 GA + 47e-4`. The vertex is at GA = 66.67 deg:
  int_n = 0.0047 + (4e-5)^2 / (4 x 3e-7) = 0.0060333 N/mm per kg/m^3.
- Density: FPL-GTR-190 Table 5-3a, "Oak, white", 12 % line: SG 0.68
  (pdftotext of `planning/extrapolation_2026-09-24/fetch/G7/pdf/fpl_gtr190_wood_handbook_2010.pdf`,
  text lines 6707-6708). The footnote (line 6866) says the SG is "based on weight when ovendry
  and volume ... at 12% moisture content". Mass at 12 % MC = ovendry mass x 1.12, so
  rho = 0.68 x 1000 x 1.12 = 761.6 kg/m^3. The same SG is in `crates/rs_cam_core/data/wood_species.toml` line 381.
- Ks = 0.0768333 x 761.6 = 58.516 N/mm^2. F_edge = 0.0060333 x 761.6 = 4.595 N/mm.
- Valid range as printed: mean chip "roughly varying from 40 um ... to 100 um" (line 155-157).
- Mean power: `P = [Ks ae ap vf + F_edge ap pi D n z psi / (2 pi)] / 6e7` kW.
  The engaged angle is `psi = acos(1 - 2 ae / D)`.
- The shear term is Ks times the removal rate. The edge term is the edge force
  times the cutting speed times the engaged fraction.
- At a constant chip and width, vf is proportional to n. Thus P is proportional to n.
- The available power is also proportional to n below 24000 rpm. Thus the power
  ratio does not change with RPM on this machine. Power never sets the RPM.
- No repo source gives the spindle or VFD efficiency. The ratio assumes the rated
  shaft power. That margin is UNSOURCED.

## Rulings (2026-11-08)

- **P1: (b)**, operator. The table grades the RPM at the binding limit now (24000 for side
  cuts, 19685 for the 6 mm drills). Feed, plunge and ramp rows stay graded at the app's own
  RPM, so only the RPM rows carry P1. The app fails these rows until P21.
- **P2: (b)**, operator. The Onsrud hardwood 60-200 row (1/4 in, 0.127-0.178 mm) stands for
  the unbranded 6 mm 2F tool (tool 0). The plunge rows of tool 0 keep the Spektra Ramp Down
  rule; P2 rules the side chip only.
- **P3: (b)**, lead, on operator delegation ("whichever is most correct"). The chart chip is
  a peak chip. The peak occurs at the largest engagement, which is the cap width
  (stepover x `trochoid_cap_mult`).
- **P4: open.** Operator reason: when the taper cuts, it does roughing work, and the tip does
  the finish. One lift for the whole edge does not hold; the two parts need their own limits.
- **Drill power (P20d Q1): advisory**, operator. The power rows of idx 3 and 4 pass when the
  app labels the figure advisory on the axial section and no gate acts on it.
- **Axial hover labels (P20d Q2)**, operator: "advance per rev" and "diameter".
- **F3000**, operator: delete `feed_max_rate` from the wanaka toolpaths. The value is the old
  `DressupConfig` default (3000.0 until S11, `74af0f8e1`). Empty = the machine cutting ceiling.

## Rulings (2026-11-10)

- **Spindle policy: constant chip, ceiling 24000**, operator ("yes, try constant chip on
  wanaka"; `P21_DESIGN.md` section 9). The project file states `spindle_strategy =
  "constant_chip"` and `spindle_rpm_ceiling = 24000` (lines 50-51). This is the policy of P1
  (b), which the table grades since 2026-11-08; the losing candidate (a) was deleted then. The
  operator ceiling 24000 is equal to the spindle max, so no expected RPM changes. The
  acceleration rule (P21 Q1) is open; the script grades no acceleration cap (lead ruling Q8).
- **idx 1 entry: helix**, operator. `entry_style = "helix"` (radius 0.3 x D = 5.4 mm, pitch
  2.0 mm). The script has a rule for a ramp feed (a full slot, lift 1) and none for a helix
  bore. The idx 1 "ramp feed" row is thus NEEDS-RULING with no candidate; the row prints the
  ramp-rule value as information. No other idx 1 row reads the entry style.

Afternoon, the six open questions (operator words in quotes). Each ruled value is a rule
that the script computes from its SRC lines; no app number is copied.

- **U1 (18 mm 2F chip): the app's stated rule**, operator: "1 and 2 sounds good". The chip is
  the Amana Spektra 1/2 in 2F row (SPK37, 0.1448 mm) x (18 / 12.7)^0.61, the generic size law
  (SIZEC) = 0.1791 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud
  straight-bit band 0.229-0.330 mm (`research/large_straight_router_bits_2026-11-10.md`); the
  tool is a straight-flute router bit. The bracket is deleted. Rows: chip, feed and entry feed
  of tool 6.
- **U3 tool 6 (18 mm plunge): the repo rule "material plunge base"**, operator (same words).
  Base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4, capped by the Z axis
  rate (PLGTXT, PLGBASE, PLGH, PLGJANKA). DERIVED from a stated repo rule; no vendor source.
- **U2 (2 mm 1F chip): the app's stated rule**, operator: "3 sounds good". Log-log
  interpolation between the printed 1.5875 and 2.38125 mm 2F rows (SIZEA, SPK12, SPK17) =
  0.0550 mm. The plunge of tool 5 is the same repo plunge rule (240 mm/min): the operator
  keeps the slow plunge, so the `n x chip` candidate (1219-1402 at 24000 rpm) is deleted.
- **U3 tool 1 (20 deg V-bit plunge) and U3 tool 4 (2 mm tapered ball plunge): the repo plunge
  rule**, operator: "for the no source ones, if they look right, let's go for it". Tool 4
  also takes the tip cap 150 mm/min per mm of tip (PLGTIP), as the app states it.
- **H1: yes**, operator. The ramp rule (chip x z x n, lift 1) grades a helix entry feed.

- **P3 width: the pass ceiling `c D`** (recorded 2026-11-10; operator ruling Q2 of 2026-11-09,
  `P22_DESIGN.md` section 9: "not a magic number; choose the most correct one"; lead: the pass
  ceiling). The lift of a geodesic Adaptive3d row reads the widest width the planner holds at
  the full feed: alpha = acos(1 - s/R) x (1 + `PASS_ENGAGEMENT_TOLERANCE` 0.05), c = (1 - cos
  alpha) / 2 (SRC PASSLIM, PASSTOL, PASSSW, PASSARC). `trochoid_cap_mult` 1.6 is retired as
  unsourced; the candidate "stepover x 1.6" is deleted. idx 1, 2, 14 and 15 are all
  `clearing_strategy = geodesic_adaptive`, so the rule covers the four adaptive rows.
- **No fallback to a stored param** (recorded 2026-11-10, lead). Where `shipped_power` is
  unmodeled (idx 5, 9) the script grades `basis.rpm.feed_mm_min`, `.value`, `.plunge_mm_min`
  and `.ramp_feed_mm_min`.

Effect of the rulings: no NEEDS-RULING row is left. Three feed rows now FAIL against a single
value where a bracket hid the difference: idx 1 and 14 (the app lifts at the nominal stepover,
P3 ruled the cap width) and idx 6 (the app lifts in a pocket that slots, check C3).
At build `0c67ad7fe` idx 1 and 14 pass (P23a: the app lifts at the pass ceiling); idx 6 still fails.

## Questions the operator must rule on

Open: P4 (the idx 16 feed row fails both candidates) and the P21 acceleration rule. U1, U2, U3
and H1 are ruled (2026-11-10); the text below is the record of each question.

- **H1. Helix entry feed (idx 1).** (a) Grade a helix as a ramp: chip x z x n, lift 1 (the
  path radius 5.4 mm is less than the tool radius 9 mm, so the contact is full width). (b) A
  different rule or a source. Ruled (a).
- **P1. RPM.** (a) The chart test RPM, 18000, where the chart prints it. (b) A
  constant chip up to the binding limit: min(spindle max 24000, XY rate / (chip x z
  x lift), Z rate / axial chip per rev for vertical moves). On this machine (b) is
  24000 for every side cut, and 19685 rpm for the 6 mm drills (Z binds:
  2500 / (0.0635 x 2)). At build 998ac93a3 the app follows (a), except idx 5 and 17 (10610 rpm, which matches neither).
- **P2. Which chart row stands for the unbranded 6 mm 2F up-cut (tool 0).** (a) Amana
  Spektra 6 mm, ".0050"" = 0.127 mm, 18000 rpm. (b) Onsrud hardwood 60-200 at 1/4 in,
  ".005-.007"" = 0.127-0.178 mm, no RPM (column read by text position). (c) Amana ZrN 2F
  flat 6 mm, "0.007"-0.009"" = 0.178-0.229 mm. At build 998ac93a3 the app uses (a) for
  idx 2, 3, 4, 15 and (b) for idx 5 and 17: one tool, one material, two chips (check C1).
- **P3. Adaptive lift width.** (a) The nominal stepover. (b) The engagement cap
  (stepover x `trochoid_cap_mult` 1.6). At (b) the lift is smaller. At build 998ac93a3
  the app uses (a). At the cap width its peak chip is above the chart chip (idx 2: 5279 / (36000 x 1.0206)
  = 0.1437 mm against 0.127 mm).
- **P4. Ball lead angle at a shallow depth (idx 16).** (a) No lift. (b) Lift 1 / sin k,
  with sin k = sqrt(1 - (1 - ap/R)^2) = 0.6 at ap 0.2 on R1.
- **U1. 18 mm 2F chip (tool 6).** No chart row. Bracket 0.1448 (1/2 in 2F, Spektra
  line 37) to 0.2286 mm (3/4 in 3F, line 54). A source for an 18 mm 2F surfacing bit is necessary.
- **U2. 2 mm 1F chip (tool 5).** No 1F row. The 2F bracket is 0.0508-0.0584 mm (Spektra
  lines 12 and 17). A rule or a source for the 1F per-tooth chip is necessary.
- **U3. Plunge rates with no source:** tool 6 (not centre-cutting, core 8 mm, protrusion
  1.0 mm), tool 1 (20 deg V-bit), tool 4 (tapered ball). Vendor plunge data is necessary.
- **U4. Drill power.** Curti covers peripheral milling only. A thrust and torque law is
  necessary, or the app must say "not modelled".

## Cross-row consistency checks (observed at build 998ac93a3; re-checked at dce570108, see the end of this section)

- **C1 (FAIL).** Tool 0 in white oak gets target chip 0.127 mm (idx 2, 15) and
  0.149 mm (idx 5, 17). A chip per tooth is a property of the tool and the material.
  One of the two is wrong under any single P2 ruling.
- **C2 (FAIL).** idx 5 and idx 17 share tool, material and rpm (10610), but the plunge is
  1347 (axial chip 0.0635 x 10610 x 2) on idx 5 and 1582 (feed / 2) on idx 17.
- **C3 (information).** idx 6 applies the ae 0.8 radial lift (1010 mm/min) in a pocket
  whose first ring is a slot. With the 2F log-log chip the slot feed is 990; 1010 is +2.0 %.

At build dce570108 (Suggest values): C1 holds (0.127 on idx 2 and 15; 0.149 on idx 5 and
17). C2 is closed in Suggest: idx 5 and 17 both print rpm 24000 and plunge 2500 (the stored
params still differ: 1347 and 1582). C3 holds at the new rpm: idx 6 prints 1347; the slot
feed is 1320; +2.0 %.

Closed (operator, 2026-11-09: "$30 is 24000 on the machine; the file is wrong"): the project
now states `[job.machine.controller] rpm_max = 24000.0` (line 94).

## Summary (generated by derive_wanaka_21.py)

108 rows (18 toolpaths x 6 quantities).

- Status: SOURCED 6, DERIVED 89, POLICY 1, UNSOURCED 2, N/A 10
- Grade: PASS 95, NEEDS-RULING 0, FAIL 3, N/A 10

FAIL rows:

- idx 6 feed: app 1347; expected P1b: 1320 // graded at app rpm 24000: @app: 1320.
- idx 16 chip per tooth (target peak): app 0.06437; expected SpeTool 2.0 mm: 0.07620.
- idx 16 feed: app 3089; expected P1b/P4a: 3658; P1b/P4b: 6096 // graded at app rpm 24000: @app/P4a: 3658; @app/P4b: 6096.

## Rows

### idx 0: Face back to T1 (tool 6)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 1/2 in x size law: 0.17910 | DERIVED | chart | 0.17913 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 8651 // graded at app rpm 24000: @app: 8651 | DERIVED | chart chip x lift; XY 15000 | 8651 | 6488 | **PASS** |
| plunge | repo plunge rule: 2163 | DERIVED | repo rule, no vendor source | 2162 | 2162 | **PASS** |
| ramp feed | P1b: 8597 // graded at app rpm 24000: @app: 8597 | DERIVED | slot chip (full-width ramp) | 8597 | 6447 | **PASS** |
| power required vs available | model: 0.05789 | DERIVED | power (ratio 0.039) | 0.05789 | - | **PASS** |

- *chip per tooth (target peak)*: U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = 0.1448 mm x (18 / 12.7)^0.61 (SIZEC) = 0.17910 mm. Repo size law, no vendor row at 18 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud straight-bit band 0.229-0.330 mm (research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit ap/D = 0.028 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (the chart of the scaled row)). n_xy = 15000 / (chip_hi 0.1791 x 2 x lift 1.0062) = 41616; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. Face: max ae = stepover 8.0, r 0.4444, lift 1.0062.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U3 tool 6, accepted as the repo rule). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(18.0 / 6) = 3.0000 -> 2162.6 mm/min; min with the Z rate 2500 (MACH) = 2162.6. The rule does not read the RPM. Tool 6 is not centre-cutting (T6CC); the rule does not read that fact.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1).
- *power required vs available*: At the app point: vf 8651, n 24000, ae 8.0000, ap 0.500, D 18.0, z 2; psi = acos(1 - 2ae/D) = 1.4595 rad; shear = Ks ae ap vf / 6e7 = 0.03375; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.02414; total 0.05789 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.039. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 1: Back bulk rough 18 mm (tool 6)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 1/2 in x size law: 0.17910 | DERIVED | chart | 0.17913 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 9122 // graded at app rpm 24000: @app: 9122 | DERIVED | chart chip x lift; XY 15000 | 9123 | 8475 | **PASS** |
| plunge | repo plunge rule: 2163 | DERIVED | repo rule, no vendor source | 2162 | 2162 | **PASS** |
| ramp feed | P1b: 8597 // graded at app rpm 24000: @app: 8597 | DERIVED | slot chip (full-width ramp) | 8598 | 6448 | **PASS** |
| power required vs available | model: 0.93999 | DERIVED | power (ratio 0.627) | 0.93999 | - | **PASS** |

- *chip per tooth (target peak)*: U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = 0.1448 mm x (18 / 12.7)^0.61 (SIZEC) = 0.17910 mm. Repo size law, no vendor row at 18 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud straight-bit band 0.229-0.330 mm (research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit ap/D = 0.556 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (the chart of the scaled row)). n_xy = 15000 / (chip_hi 0.1791 x 2 x lift 1.0610) = 39466; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 with ruling Q2 (2026-11-09): the width is the pass ceiling c D of the stepover 5.500 (clearing_strategy geodesic_adaptive): alpha = acos(1 - s/R) x 1.05 (PASSARC, PASSTOL), c = (1 - cos alpha) / 2 = 0.33285, c D = 5.9913 mm, lift 1.0610 (the nominal stepover would give lift 1.0854). Note:  Shipped 8475 (at 18000 rpm) is the operator value: P at the ceiling width 5.99 mm = 0.800 kW = 71.1 % of 1.125 kW.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U3 tool 6, accepted as the repo rule). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(18.0 / 6) = 3.0000 -> 2162.6 mm/min; min with the Z rate 2500 (MACH) = 2162.6. The rule does not read the RPM. Tool 6 is not centre-cutting (T6CC); the rule does not read that fact.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1). The entry is a helix (r = 0.3 x D = 5.40 mm, pitch 2.00 mm, slope 3.37 deg). H1 ruled "yes", operator 2026-11-10: the ramp rule grades a helix entry feed. The path radius is less than the tool radius 9.0 mm, so the contact is full width.
- *power required vs available*: At the app point: vf 9123, n 24000, ae 5.9913, ap 10.000, D 18.0, z 2; psi = acos(1 - 2ae/D) = 1.2299 rad; shear = Ks ae ap vf / 6e7 = 0.53307; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.40691; total 0.93999 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.627. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 2: Back rest 6 mm (cavity, hollow, lakes, pockets) (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Onsrud 60-200 1/4 in: 0.12700-0.17780 | DERIVED | chart | 0.12700 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 6842-9578 // graded at app rpm 24000: @app: 6842-9578 | DERIVED | chart chip x lift; XY 15000 | 6841 | 5279 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 24000: @app: 2500 | DERIVED | Z rate 2500 | 2500 | 2285 | **PASS** |
| ramp feed | P1b: 6096-8534 // graded at app rpm 24000: @app: 6096-8534 | DERIVED | slot chip (full-width ramp) | 6095 | 4571 | **PASS** |
| power required vs available | model: 0.13833 | DERIVED | power (ratio 0.092) | 0.13833 | - | **PASS** |

- *chip per tooth (target peak)*: ONS46 ".005-.007" x 25.4 = 0.1270-0.1778 mm | P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for the unbranded tool. Onsrud column read by text position (I). ap/D = 1.000 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)). n_xy = 15000 / (chip_hi 0.1778 x 2 x lift 1.1223) = 37585; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 with ruling Q2 (2026-11-09): the width is the pass ceiling c D of the stepover 1.500 (clearing_strategy geodesic_adaptive): alpha = acos(1 - s/R) x 1.05 (PASSARC, PASSTOL), c = (1 - cos alpha) / 2 = 0.27300, c D = 1.6380 mm, lift 1.1223 (the nominal stepover would give lift 1.1547).
- *plunge*: SPK26 Ramp Down 90" = 2286.0 mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = 3048 > Z 2500 -> 2500. At the app rpm: 0.127 x n.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1). Vertical part: 4572 x sin 3deg = 239 mm/min < Ramp Down 2286: the side chip binds.
- *power required vs available*: At the app point: vf 6841, n 24000, ae 1.6380, ap 6.000, D 6.0, z 2; psi = acos(1 - 2ae/D) = 1.0996 rad; shear = Ks ae ap vf / 6e7 = 0.06557; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.07276; total 0.13833 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.092. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 3: Pin drill, flip pins (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| axial chip per tooth | SPK26: 0.06350 | SOURCED | chart | 0.06350 | - | **PASS** |
| rpm | Z-bound: 19685 | DERIVED | Z rate | 19685 | 18000 | **PASS** |
| feed (drill, vertical) | Z-bound: 2500 // graded at app rpm 19685: @app: 2500 | DERIVED | Z rate | 2499 | 1000 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 19685: @app: 2500 | DERIVED | Z rate | 2499 | - | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | none | DERIVED | advisory only | 0.15416 | - | **PASS** |

- *axial chip per tooth*: SPK26 Ramp Down "90"" x 25.4 = 2286.0 mm/min; / (18000 x 2) = 0.06350 mm (SPK3 18,000 RPM; SPK52 Ramp Down = Feed / flutes) Note: App chip = vendor_band.score_winner_chip_mm.
- *rpm*: P1 ruled (b), operator 2026-11-08. Constant axial chip; vz = 0.0635 x 2 x n <= 2500 -> n <= 19685; < 24000, so the Z rate binds.
- *feed (drill, vertical)*: P1 (b): 0.06350 x 2 x 19685 = 2500 = Z rate 2500 (SPK26 Ramp Down 90" = 2286.0 mm/min at 18000). Graded at the app rpm: axial chip x 2 x n. Note: Shipped 1000 mm/min is an operator value (feeds_lock spindle).
- *plunge*: A drill cycle has one feed; this row repeats the feed row.
- *ramp feed*: A drill cycle has no ramp.
- *power required vs available*: Curti 2021 models peripheral milling only (CUR672). No repo source gives an axial drilling force for wood. Ruling 2026-11-08: the app may print the axial-section figure only as advisory (shipped_power.advisory = true, section = axial_circle). Note: Graded on the label: advisory = True, section = axial_circle.

### idx 4: Drill light holes 6 mm end mill (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| axial chip per tooth | SPK26: 0.06350 | SOURCED | chart | 0.06350 | - | **PASS** |
| rpm | Z-bound: 19685 | DERIVED | Z rate | 19685 | 18000 | **PASS** |
| feed (drill, vertical) | Z-bound: 2500 // graded at app rpm 19685: @app: 2500 | DERIVED | Z rate | 2499 | 1000 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 19685: @app: 2500 | DERIVED | Z rate | 2499 | - | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | none | DERIVED | advisory only | 0.15416 | - | **PASS** |

- *axial chip per tooth*: SPK26 Ramp Down "90"" x 25.4 = 2286.0 mm/min; / (18000 x 2) = 0.06350 mm (SPK3 18,000 RPM; SPK52 Ramp Down = Feed / flutes) Note: App chip = vendor_band.score_winner_chip_mm.
- *rpm*: P1 ruled (b), operator 2026-11-08. Constant axial chip; vz = 0.0635 x 2 x n <= 2500 -> n <= 19685; < 24000, so the Z rate binds.
- *feed (drill, vertical)*: P1 (b): 0.06350 x 2 x 19685 = 2500 = Z rate 2500 (SPK26 Ramp Down 90" = 2286.0 mm/min at 18000). Graded at the app rpm: axial chip x 2 x n. Note: Shipped 1000 mm/min is an operator value (feeds_lock spindle).
- *plunge*: A drill cycle has one feed; this row repeats the feed row.
- *ramp feed*: A drill cycle has no ramp.
- *power required vs available*: Curti 2021 models peripheral milling only (CUR672). No repo source gives an axial drilling force for wood. Ruling 2026-11-08: the app may print the axial-section figure only as advisory (shipped_power.advisory = true, section = axial_circle). Note: Graded on the label: advisory = True, section = axial_circle.

### idx 5: River channel 6 mm, pass 7 of 7 (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Onsrud 60-200 1/4 in: 0.12700-0.17780 | DERIVED | chart | 0.14919 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 10610 | **PASS** |
| feed | P1b: 6096-8534 // graded at app rpm 24000: @app: 6096-8534 | DERIVED | chart chip x lift; XY 15000 | 7161 | 3165 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 24000: @app: 2500 | DERIVED | Z rate 2500 | 2500 | 1347 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | none | UNSOURCED | none | - | - | **PASS** |

- *chip per tooth (target peak)*: ONS46 ".005-.007" x 25.4 = 0.1270-0.1778 mm | P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for the unbranded tool. Onsrud column read by text position (I). Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)). n_xy = 15000 / (chip_hi 0.1778 x 2 x lift 1.0000) = 42182; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1.
- *plunge*: SPK26 Ramp Down 90" = 2286.0 mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = 3048 > Z 2500 -> 2500. At the app rpm: 0.127 x n.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: The depth per pass comes from the simulated surface; no fixed ap exists to evaluate. The app declares 'unmodeled (DepthAfterSimulation)'.

### idx 6: Lake pocket 2 mm, 11 lakes to Z 5.88 (tool 5)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 2F log-log: 0.05501 | DERIVED | chart | 0.05501 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 1320 // graded at app rpm 24000: @app: 1320 | DERIVED | chart chip x lift; XY 15000 | 1347 | 1010 | **FAIL** |
| plunge | repo plunge rule: 240 | DERIVED | repo rule, no vendor source | 240 | 240 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | model: 0.00588 | DERIVED | power (ratio 0.004) | 0.00588 | - | **PASS** |

- *chip per tooth (target peak)*: U2 ruled, operator 2026-11-10. Log-log interpolation (SIZEA) between SPK12 1/16 in ".0020"" = 0.0508 mm and SPK17 3/32 in ".0023"" = 0.0584 mm at 2.0 mm = 0.05501 mm. The rows are 2F; the tool is 1F; the operator accepts the 2F chip per tooth for it ap/D = 0.350 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM). n_xy = 15000 / (chip_hi 0.0550 x 1 x lift 1.0000) = 272677; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1. Note:  With the 2F log-log chip 0.05501 in a slot the feed is 1320 at the app rpm; the app's 1347 is +2.0 %: it applies the ae 0.8 lift in a pocket that slots.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U2, the operator keeps the slow plunge; the n x chip candidate is deleted). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(2.0 / 6) = 0.3333 -> 240.3 mm/min; min with the Z rate 2500 (MACH) = 240.3. The rule does not read the RPM.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: At the app point: vf 1347, n 24000, ae 2.0000, ap 0.700, D 2.0, z 1; psi = acos(1 - 2ae/D) = 3.1416 rad; shear = Ks ae ap vf / 6e7 = 0.00184; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.00404; total 0.00588 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.004. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 7: Lake pocket 2 mm, 1 lake to Z 3.99 (tool 5)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 2F log-log: 0.05501 | DERIVED | chart | 0.05501 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 1320 // graded at app rpm 24000: @app: 1320 | DERIVED | chart chip x lift; XY 15000 | 1320 | 1258 | **PASS** |
| plunge | repo plunge rule: 240 | DERIVED | repo rule, no vendor source | 240 | 240 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | model: 0.00417 | DERIVED | power (ratio 0.003) | 0.00417 | - | **PASS** |

- *chip per tooth (target peak)*: U2 ruled, operator 2026-11-10. Log-log interpolation (SIZEA) between SPK12 1/16 in ".0020"" = 0.0508 mm and SPK17 3/32 in ".0023"" = 0.0584 mm at 2.0 mm = 0.05501 mm. The rows are 2F; the tool is 1F; the operator accepts the 2F chip per tooth for it ap/D = 0.250 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM). n_xy = 15000 / (chip_hi 0.0550 x 1 x lift 1.0000) = 272677; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U2, the operator keeps the slow plunge; the n x chip candidate is deleted). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(2.0 / 6) = 0.3333 -> 240.3 mm/min; min with the Z rate 2500 (MACH) = 240.3. The rule does not read the RPM.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: At the app point: vf 1320, n 24000, ae 2.0000, ap 0.500, D 2.0, z 1; psi = acos(1 - 2ae/D) = 3.1416 rad; shear = Ks ae ap vf / 6e7 = 0.00129; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.00289; total 0.00417 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.003. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 8: ANTENNA_D5.00 pocket 2 mm (tool 5)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 2F log-log: 0.05501 | DERIVED | chart | 0.05501 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 1320 // graded at app rpm 24000: @app: 1320 | DERIVED | chart chip x lift; XY 15000 | 1320 | 1258 | **PASS** |
| plunge | repo plunge rule: 240 | DERIVED | repo rule, no vendor source | 240 | 240 | **PASS** |
| ramp feed | P1b: 1320 // graded at app rpm 24000: @app: 1320 | DERIVED | slot chip (full-width ramp) | 1320 | 1258 | **PASS** |
| power required vs available | model: 0.00417 | DERIVED | power (ratio 0.003) | 0.00417 | - | **PASS** |

- *chip per tooth (target peak)*: U2 ruled, operator 2026-11-10. Log-log interpolation (SIZEA) between SPK12 1/16 in ".0020"" = 0.0508 mm and SPK17 3/32 in ".0023"" = 0.0584 mm at 2.0 mm = 0.05501 mm. The rows are 2F; the tool is 1F; the operator accepts the 2F chip per tooth for it ap/D = 0.250 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM). n_xy = 15000 / (chip_hi 0.0550 x 1 x lift 1.0000) = 272677; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U2, the operator keeps the slow plunge; the n x chip candidate is deleted). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(2.0 / 6) = 0.3333 -> 240.3 mm/min; min with the Z rate 2500 (MACH) = 240.3. The rule does not read the RPM.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1).
- *power required vs available*: At the app point: vf 1320, n 24000, ae 2.0000, ap 0.500, D 2.0, z 1; psi = acos(1 - 2ae/D) = 3.1416 rad; shear = Ks ae ap vf / 6e7 = 0.00129; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.00289; total 0.00417 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.003. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 9: V rivers and lakes (tool 1)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | AMS 18/30 deg 1F: 0.07620-0.17780 | SOURCED | chart | 0.12700 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 1829-4267 // graded at app rpm 24000: @app: 1829-4267 | DERIVED | chart chip x lift; XY 15000 | 3048 | 2286 | **PASS** |
| plunge | repo plunge rule: 381 | DERIVED | repo rule, no vendor source | 381 | 381 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | none | UNSOURCED | none | - | - | **PASS** |

- *chip per tooth (target peak)*: AMS11 18deg and 30deg 1F both "0.003" - 0.007""; 20deg lies between equal cells: 0.0762-0.1778 mm Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: AMS3 18,000). n_xy = 15000 / (chip_hi 0.1778 x 1 x lift 1.0000) = 84364; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. V trace is a slot; the V chart is for the same geometry: lift 1. Printed IPM: 50"-130" = 1270-3302 mm/min at 18000.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: "for the no source ones, if they look right, let's go for it"). PLGTXT: 1000 / h at 6 mm x clamp(D/6, 0.25, 3); h = 1.3873 (Janka 1360, PLGJANKA); x clamp(3.175 / 6) = 0.5292 -> 381.4 mm/min; Z rate 2500 (MACH); min = 381.4. No repo source prints a plunge for a 20deg 1F V-bit.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: The depth per pass comes from the simulated surface; no fixed ap exists to evaluate. The app declares 'unmodeled (DepthAfterSimulation)'.

### idx 10: Day strip slit V60 (tool 2)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | AMS 60 deg 2F: 0.06350-0.07620 | SOURCED | chart | 0.06350 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 3048-3658 // graded at app rpm 24000: @app: 3048-3658 | DERIVED | chart chip x lift; XY 15000 | 3048 | 2286 | **PASS** |
| plunge | IDC ratio x feed (P1 b): 1016-1829 // graded at app rpm 24000: @app: 1016-1829 | DERIVED | cross-vendor ratio | 1016 | 762 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | model: 0.03762 | DERIVED | power (ratio 0.025) | 0.03762 | - | **PASS** |

- *chip per tooth (target peak)*: AMS11 60deg 2F: "90"" / (18000 x 2) x 25.4 = 0.0635 mm; printed "0.003"" = 0.0762 mm; the cell disagrees with itself ap/D = 0.409 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: AMS3 18,000). n_xy = 15000 / (chip_hi 0.0762 x 2 x lift 1.0000) = 98425; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. V trace is a slot; the V chart is for the same geometry: lift 1. Printed IPM: 90" = 2286 mm/min at 18000.
- *plunge*: No Amana plunge for this bit. IDC 60deg 2F V-bit prints plunge/feed 20/40 = 0.5 (IDC165) and 20/60 = 0.333 (IDC285); the two printings disagree. Range = [feed_lo/3, feed_hi/2]: a [2286/3, 2743/2].
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: V-groove at the app point: vf 3048, n 24000, ap 2.60, R = ap tan 30 = 1.5011; shear Ks x ap x R x vf = 0.01160; edge F_edge x pi n R^2 / sin 30 x z / 2 = 0.02602; total 0.03762 kW. The app uses a rectangle ae 6.35 x ap. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.025. Spindle efficiency: no source; the ratio assumes the rated shaft power. Note: App overstates the V cross-section (rectangle, not triangle): conservative.

### idx 11: Face clamp pads 6 mm (tool 6)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 1/2 in x size law: 0.17910 | DERIVED | chart | 0.17913 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 8597 // graded at app rpm 24000: @app: 8597 | DERIVED | chart chip x lift; XY 15000 | 8651 | 6488 | **PASS** |
| plunge | repo plunge rule: 2163 | DERIVED | repo rule, no vendor source | 2162 | 2162 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | model: 0.12790 | DERIVED | power (ratio 0.085) | 0.12790 | - | **PASS** |

- *chip per tooth (target peak)*: U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = 0.1448 mm x (18 / 12.7)^0.61 (SIZEC) = 0.17910 mm. Repo size law, no vendor row at 18 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud straight-bit band 0.229-0.330 mm (research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit ap/D = 0.028 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (the chart of the scaled row)). n_xy = 15000 / (chip_hi 0.1791 x 2 x lift 1.0000) = 41875; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U3 tool 6, accepted as the repo rule). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(18.0 / 6) = 3.0000 -> 2162.6 mm/min; min with the Z rate 2500 (MACH) = 2162.6. The rule does not read the RPM. Tool 6 is not centre-cutting (T6CC); the rule does not read that fact. Note: Straight plunge entry at ap 0.5 <= protrusion 1.0: geometrically possible.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: At the app point: vf 8651, n 24000, ae 18.0000, ap 0.500, D 18.0, z 2; psi = acos(1 - 2ae/D) = 3.1416 rad; shear = Ks ae ap vf / 6e7 = 0.07593; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.05197; total 0.12790 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.085. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 12: Face front to T (tool 6)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 1/2 in x size law: 0.17910 | DERIVED | chart | 0.17913 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 8651 // graded at app rpm 24000: @app: 8651 | DERIVED | chart chip x lift; XY 15000 | 8651 | 6488 | **PASS** |
| plunge | repo plunge rule: 2163 | DERIVED | repo rule, no vendor source | 2162 | 2162 | **PASS** |
| ramp feed | P1b: 8597 // graded at app rpm 24000: @app: 8597 | DERIVED | slot chip (full-width ramp) | 8597 | 6447 | **PASS** |
| power required vs available | model: 0.05789 | DERIVED | power (ratio 0.039) | 0.05789 | - | **PASS** |

- *chip per tooth (target peak)*: U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = 0.1448 mm x (18 / 12.7)^0.61 (SIZEC) = 0.17910 mm. Repo size law, no vendor row at 18 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud straight-bit band 0.229-0.330 mm (research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit ap/D = 0.028 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (the chart of the scaled row)). n_xy = 15000 / (chip_hi 0.1791 x 2 x lift 1.0062) = 41616; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. Face: max ae = stepover 8.0, r 0.4444, lift 1.0062.
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U3 tool 6, accepted as the repo rule). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(18.0 / 6) = 3.0000 -> 2162.6 mm/min; min with the Z rate 2500 (MACH) = 2162.6. The rule does not read the RPM. Tool 6 is not centre-cutting (T6CC); the rule does not read that fact.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1).
- *power required vs available*: At the app point: vf 8651, n 24000, ae 8.0000, ap 0.500, D 18.0, z 2; psi = acos(1 - 2ae/D) = 1.4595 rad; shear = Ks ae ap vf / 6e7 = 0.03375; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.02414; total 0.05789 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.039. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 13: Engrave day labels V60 (tool 2)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | AMS 60 deg 2F: 0.06350-0.07620 | SOURCED | chart | 0.06350 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 3048-3658 // graded at app rpm 24000: @app: 3048-3658 | DERIVED | chart chip x lift; XY 15000 | 3048 | 2286 | **PASS** |
| plunge | IDC ratio x feed (P1 b): 1016-1829 // graded at app rpm 24000: @app: 1016-1829 | DERIVED | cross-vendor ratio | 1016 | 762 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | model: 0.00200 | DERIVED | power (ratio 0.001) | 0.00200 | - | **PASS** |

- *chip per tooth (target peak)*: AMS11 60deg 2F: "90"" / (18000 x 2) x 25.4 = 0.0635 mm; printed "0.003"" = 0.0762 mm; the cell disagrees with itself ap/D = 0.094 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: AMS3 18,000). n_xy = 15000 / (chip_hi 0.0762 x 2 x lift 1.0000) = 98425; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. V trace is a slot; the V chart is for the same geometry: lift 1. Printed IPM: 90" = 2286 mm/min at 18000.
- *plunge*: No Amana plunge for this bit. IDC 60deg 2F V-bit prints plunge/feed 20/40 = 0.5 (IDC165) and 20/60 = 0.333 (IDC285); the two printings disagree. Range = [feed_lo/3, feed_hi/2]: a [2286/3, 2743/2].
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: V-groove at the app point: vf 3048, n 24000, ap 0.60, R = ap tan 30 = 0.3464; shear Ks x ap x R x vf = 0.00062; edge F_edge x pi n R^2 / sin 30 x z / 2 = 0.00139; total 0.00200 kW. The app uses a rectangle ae 6.35 x ap. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.001. Spindle efficiency: no source; the ratio assumes the rated shaft power. Note: App overstates the V cross-section (rectangle, not triangle): conservative.

### idx 14: Front bulk rough 18 mm (tool 6)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Spektra 1/2 in x size law: 0.17910 | DERIVED | chart | 0.17913 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 10396 // graded at app rpm 24000: @app: 10396 | DERIVED | chart chip x lift; XY 15000 | 10397 | 8060 | **PASS** |
| plunge | repo plunge rule: 2163 | DERIVED | repo rule, no vendor source | 2162 | 2162 | **PASS** |
| ramp feed | P1b: 8597 // graded at app rpm 24000: @app: 8597 | DERIVED | slot chip (full-width ramp) | 8597 | 6448 | **PASS** |
| power required vs available | model: 0.30061 | DERIVED | power (ratio 0.200) | 0.30061 | - | **PASS** |

- *chip per tooth (target peak)*: U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = 0.1448 mm x (18 / 12.7)^0.61 (SIZEC) = 0.17910 mm. Repo size law, no vendor row at 18 mm. It lies inside the old bracket 0.1448-0.2286 mm and under the Freud straight-bit band 0.229-0.330 mm (research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit ap/D = 0.231 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (the chart of the scaled row)). n_xy = 15000 / (chip_hi 0.1791 x 2 x lift 1.2093) = 34629; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 with ruling Q2 (2026-11-09): the width is the pass ceiling c D of the stepover 3.600 (clearing_strategy geodesic_adaptive): alpha = acos(1 - s/R) x 1.05 (PASSARC, PASSTOL), c = (1 - cos alpha) / 2 = 0.21886, c D = 3.9395 mm, lift 1.2093 (the nominal stepover would give lift 1.2500).
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: U3 tool 6, accepted as the repo rule). PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = 1.3873; 1000 / h = 720.9; x clamp(18.0 / 6) = 3.0000 -> 2162.6 mm/min; min with the Z rate 2500 (MACH) = 2162.6. The rule does not read the RPM. Tool 6 is not centre-cutting (T6CC); the rule does not read that fact.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1).
- *power required vs available*: At the app point: vf 10397, n 24000, ae 3.9395, ap 4.166, D 18.0, z 2; psi = acos(1 - 2ae/D) = 0.9737 rad; shear = Ks ae ap vf / 6e7 = 0.16642; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.13420; total 0.30061 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.200. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 15: Front 3D rough (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Onsrud 60-200 1/4 in: 0.12700-0.17780 | DERIVED | chart | 0.12700 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b: 7023-9832 // graded at app rpm 24000: @app: 7023-9832 | DERIVED | chart chip x lift; XY 15000 | 7022 | 5429 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 24000: @app: 2500 | DERIVED | Z rate 2500 | 2500 | 2285 | **PASS** |
| ramp feed | P1b: 6096-8534 // graded at app rpm 24000: @app: 6096-8534 | DERIVED | slot chip (full-width ramp) | 6095 | 4571 | **PASS** |
| power required vs available | model: 0.12135 | DERIVED | power (ratio 0.081) | 0.12135 | - | **PASS** |

- *chip per tooth (target peak)*: ONS46 ".005-.007" x 25.4 = 0.1270-0.1778 mm | P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for the unbranded tool. Onsrud column read by text position (I). ap/D = 0.922 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)). n_xy = 15000 / (chip_hi 0.1778 x 2 x lift 1.1520) = 36615; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 with ruling Q2 (2026-11-09): the width is the pass ceiling c D of the stepover 1.382 (clearing_strategy geodesic_adaptive): alpha = acos(1 - s/R) x 1.05 (PASSARC, PASSTOL), c = (1 - cos alpha) / 2 = 0.25174, c D = 1.5104 mm, lift 1.1520 (the nominal stepover would give lift 1.1875).
- *plunge*: SPK26 Ramp Down 90" = 2286.0 mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = 3048 > Z 2500 -> 2500. At the app rpm: 0.127 x n.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1). Vertical part: 4572 x sin 3deg = 239 mm/min < Ramp Down 2286: the side chip binds.
- *power required vs available*: At the app point: vf 7022, n 24000, ae 1.5104, ap 5.532, D 6.0, z 2; psi = acos(1 - 2ae/D) = 1.0512 rad; shear = Ks ae ap vf / 6e7 = 0.05722; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.06413; total 0.12135 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.081. Spindle efficiency: no source; the ratio assumes the rated shaft power.

### idx 16: Front scallop finish (tool 4)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | SpeTool 2.0 mm: 0.07620 | SOURCED | chart | 0.06437 | - | **FAIL** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 18000 | **PASS** |
| feed | P1b/P4a: 3658; P1b/P4b: 6096 // graded at app rpm 24000: @app/P4a: 3658; @app/P4b: 6096 | POLICY | chart chip x lift; XY 15000 | 3089 | 2317 | **FAIL** |
| plunge | repo plunge rule: 240 | DERIVED | repo rule, no vendor source | 240 | 240 | **PASS** |
| ramp feed | none | N/A | none | - | - | **N/A** |
| power required vs available | <= bound: 0.00000-0.00143 | DERIVED | power (ratio 0.001) | 0.00143 | - | **PASS** |

- *chip per tooth (target peak)*: SPE23 chipload "0.003" (inch, 2 flutes derived: 108/(18000 x 0.003) = 2.00) x 25.4 = 0.0762 mm ap/D = 0.100 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPE23 18,000). n_xy = 15000 / (chip_hi 0.0762 x 2 x lift 1.6667) = 59055; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. Ball R 1.0 at ap 0.20: sin k = sqrt(1-(1-ap/R)^2) = 0.6000; peak chip = fz sin k. P4b lift = 1/0.6000 = 1.6667. The chart chip is at 1 x D (SPE14), where the equator cuts (sin k = 1).
- *plunge*: Repo rule, no vendor source (operator ruling 2026-11-10: "for the no source ones, if they look right, let's go for it"). PLGTXT: 1000 / h at 6 mm x clamp(D/6, 0.25, 3); h = 1.3873 (Janka 1360, PLGJANKA); x clamp(2.000 / 6) = 0.3333 -> 240.3 mm/min; Z rate 2500 (MACH); tip cap 150 x tip 2.0 mm = 300 mm/min (PLGTIP); min = 240.3. SpeTool (SPE23) and Amana ZrN print no plunge.
- *ramp feed*: The entry is a straight plunge; no ramp.
- *power required vs available*: Upper bound: rectangle ae x ap on the engaged diameter 2 sqrt(R^2-(R-ap)^2) = 1.200 mm; shear 0.000525 + edge 0.000900 = 0.001426 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.001. Spindle efficiency: no source; the ratio assumes the rated shaft power. Note: Bound only; the scallop cross-section is smaller than the rectangle.

### idx 17: Outline profile with tabs (tool 0)

| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |
|---|---|---|---|---|---|---|
| chip per tooth (target peak) | Onsrud 60-200 1/4 in: 0.12700-0.17780 | DERIVED | chart | 0.14919 | - | **PASS** |
| rpm | P1b binding limit: 24000 | DERIVED | spindle max 24000 | 24000 | 10610 | **PASS** |
| feed | P1b: 6096-8534 // graded at app rpm 24000: @app: 6096-8534 | DERIVED | chart chip x lift; XY 15000 | 7161 | 3165 | **PASS** |
| plunge | Z-bound: 2500 // graded at app rpm 24000: @app: 2500 | DERIVED | Z rate 2500 | 2500 | 1582 | **PASS** |
| ramp feed | P1b: 6096-8534 // graded at app rpm 24000: @app: 6096-8534 | DERIVED | slot chip (full-width ramp) | 7161 | 3165 | **PASS** |
| power required vs available | model: 0.09569 | DERIVED | power (ratio 0.064) | 0.09569 | - | **PASS** |

- *chip per tooth (target peak)*: ONS46 ".005-.007" x 25.4 = 0.1270-0.1778 mm | P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for the unbranded tool. Onsrud column read by text position (I). ap/D = 0.208 <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate. Note: App = vendor_band.score_winner_chip_mm (the target before the radial lift).
- *rpm*: P1 ruled (b), operator 2026-11-08 (the chart rpm: SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)). n_xy = 15000 / (chip_hi 0.1778 x 2 x lift 1.0000) = 42182; spindle max 24000; n_b = 24000 (spindle max 24000). Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.
- *feed*: feed = chip x z x n x lift (SPK47), clamped to XY 15000. The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1.
- *plunge*: SPK26 Ramp Down 90" = 2286.0 mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = 3048 > Z 2500 -> 2500. At the app rpm: 0.127 x n.
- *ramp feed*: A ramp cuts a full slot: ramp feed = chip x z x n (lift 1). Vertical part: 4572 x sin 3deg = 239 mm/min < Ramp Down 2286: the side chip binds.
- *power required vs available*: At the app point: vf 7161, n 24000, ae 6.0000, ap 1.250, D 6.0, z 2; psi = acos(1 - 2ae/D) = 3.1416 rad; shear = Ks ae ap vf / 6e7 = 0.05238; edge = F_edge ap pi D n z psi / 2pi / 6e7 = 0.04331; total 0.09569 kW. P_avail = 1.5 x 24000 / 24000 = 1.5000 kW (MACH); ratio 0.064. Spindle efficiency: no source; the ratio assumes the rated shaft power.
