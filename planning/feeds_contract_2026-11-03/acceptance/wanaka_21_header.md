# Wanaka 21: feeds and speeds acceptance table

This table states what each feeds-and-speeds number of the wanaka 21 project
SHOULD be. Each expected value comes from a printed source line in the repo,
the project file, or arithmetic shown in the row. The app is the thing under
test. No expected value comes from the engine code or from the app output.

- Project: `planning/rivmap_block_job/nz_wanaka_21_operator.toml`, 18 toolpaths, white oak.
- Graded build: `dce570108` (P21c; commit time 1794272543, 2026-11-10 14:02). The running
  `rs_cam_gui` binary (`target/release`) has mtime 1794270915 (13:35), EARLIER than the commit;
  the process started 2026-11-10 13:54. Lead statement: the binary was built from the P21c
  working tree shortly before the commit, and after that build only test files, the
  literature-matrix shim and one snapshot changed (no product source). The README step 1 rule
  (binary later than HEAD) is thus not met to the letter; the lead ruled to grade. HEAD at the
  read was `3898f1b82` (a planning-only commit after `dce570108`); no edits under `crates/`.
  Graded by a fresh agent on 2026-11-10: PASS 64, FAIL 6, NEEDS-RULING 28, N/A 10. After the
  operator rulings of the afternoon (same build, same app values; `grade_dce570108_r2.md`):
  PASS 92, FAIL 6, NEEDS-RULING 0, N/A 10. The
  previous grade (build `b9e67b24e`, P20e, 2026-11-09) was PASS 47, FAIL 23, NEEDS-RULING 28,
  N/A 10; build `4b7aa0e96` (2026-11-08) had the same counts; build `998ac93a3` (before the
  rulings of 2026-11-08) was PASS 31, FAIL 12, NEEDS-RULING 55.
- App values: MCP `get_suggest_rationale` (Suggest) and `get_toolpath_params`
  (shipped), read on 2026-11-10. Nobody applied Suggest after P21c, so each shipped param
  still holds its old value (18000 rpm; 10610 rpm on idx 5 and 17). The Suggest column is
  the graded value. Suggest prints the RPM in `basis.rpm.value`, also for idx 5 and 9, where
  `shipped_power` is unmodeled.
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

Effect of the rulings: no NEEDS-RULING row is left. Three feed rows now FAIL against a single
value where a bracket hid the difference: idx 1 and 14 (the app lifts at the nominal stepover,
P3 ruled the cap width) and idx 6 (the app lifts in a pocket that slots, check C3).

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

