# P26 survey: what the Pocket width rule gives a 3D adaptive rough

Measurement only, 2026-11-11. No product code. One temporary probe test ran through the
session doors (`ProjectSession::load`, `SetPostConfig`, `safe_envelope`) and is deleted. It
loaded `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml` (the 6 mm dial-in) and
`nz_wanaka_21_operator.toml` (the 18 mm tool). Material: white oak. Policy: constant chip.

## 1. Where the width rule lives, and why Adaptive does not get it

- The rule: `SafeEnvelope::propose_removal_rate` in `crates/rs_cam_core/src/feeds/envelope.rs`.
  It picks the width and depth with the most `ae x ap x feed` inside every limit.
- The switch: `AxesPlan::computed_width` (same file, in `axes_plan`). It is true only for
  `family == OperationFamily::Pocket && role == PassRole::Roughing` with a stated-width radial.
  Face, Pocket and Zigzag pass. Adaptive and 3D Adaptive are `OperationFamily::Adaptive`.
- Two more things follow the same family split:
  - The corner-coverage limit (`LimitKind::CornerCoverage`, width <= 0.8536 D) is added only
    when `computed_width` is true.
  - The depth limit: `RigidityProfile::depth_cap_rule` gives the Adaptive family
    `DepthCapRule::Factor(adaptive_doc_factor)` (2 x D). Every other roughing family gets
    `DepthCapRule::ForceBudget` (35.3 N on this machine).
- The ruling is P13c, recorded in `phases/P13.md`: "The Adaptive family keeps
  `adaptive_doc_factor`." and "Adaptive and finishing ops keep `propose` (min(stated, binding
  cap))." The same note has a paragraph "Adaptive (report only)" with force-budget depths. So
  the exclusion was a scope decision of P13c, not a missing limit found later.

Result today for a 3D Adaptive: the only width limits are the diameter (ceiling) and the move
budget (floor). The proposal is the stated stepover.

## 2. What the Pocket rule gives for the two wanaka cuts

The probe built a Pocket op with the same tool, material, machine, policy and stated depth,
and no stepover. "Today" is the stored 3D Adaptive op.

### 2.1 The 6 mm 2-flute flat end mill (chip 0.127 mm)

| | Width (mm) | Depth (mm) | Feed (mm/min) | RPM | Removal (mm3/min) | Limits |
|---|---|---|---|---|---|---|
| idx 1 today, ceiling 24000 | 1.20 | 6.0 | 7372 | 24000 | 53 076 | none binds |
| idx 1 Pocket rule, 24000 | 5.12 (0.854 D) | 3.00 | 6096 | 24000 | 93 675 | force budget (depth), corner coverage (width) |
| idx 1 today, 18000 | 1.20 | 6.0 | 5529 | 18000 | 39 807 | none binds |
| idx 1 Pocket rule, 18000 | 5.12 | 3.00 | 4572 | 18000 | 70 256 | force budget, corner coverage |
| idx 14 today, 24000 | 1.20 | 3.0 | 7372 | 24000 | 26 538 | none binds |
| idx 14 Pocket rule, 24000 | 5.12 | 3.0 (stated) | 6096 | 24000 | 93 659 | corner coverage |
| idx 14 today, 18000 | 1.20 | 3.0 | 5529 | 18000 | 19 904 | none binds |
| idx 14 Pocket rule, 18000 | 5.12 | 3.0 | 4572 | 18000 | 70 244 | corner coverage |

Power at the Pocket pick: 0.21 kW of 1.5 kW. Power, deflection, the pass ceiling and the
vendor axial bound do not bind. The Pocket rule lowers the idx 1 depth from 6.0 to 3.00 mm:
the stated depth is above the force budget at every width from 1.2 mm up.

Removal along the width, idx 1, ceiling 24000. "Budget depth" is the force-budget ceiling at
that width, capped at the stated 6 mm.

| Width (mm) | Share of D | Feed | Budget depth (mm) | Removal at the budget depth | Removal at 6 mm (outside the budget from 1.2 mm) |
|---|---|---|---|---|---|
| 0.6 | 0.10 | 10160 | 6.00 | 36 576 | 36 576 |
| 0.9 | 0.15 | 8536 | 6.00 | 46 095 | 46 095 |
| 1.2 | 0.20 | 7620 | 3.85 | 35 163 | 54 864 |
| 1.5 | 0.25 | 7039 | 3.62 | 38 206 | 63 351 |
| 1.8 | 0.30 | 6651 | 3.44 | 41 234 | 71 834 |
| 2.4 | 0.40 | 6222 | 3.24 | 48 379 | 89 593 |
| 3.0 | 0.50 | 6096 | 3.09 | 56 541 | 109 728 |
| 3.6 | 0.60 | 6096 | 3.01 | 66 080 | 131 674 |
| 4.2 | 0.70 | 6096 | 3.00 | 76 823 | 153 619 |
| 4.8 | 0.80 | 6096 | 3.00 | 87 797 | 175 565 |
| 5.12 | 0.854 | 6096 | 3.00 | 93 675 (the pick) | 187 328 |
| 6.0 | 1.00 | 6096 | 3.00 | 109 737 (past the corner bound) | 219 456 |

Two things are visible. There is a local best at about 0.9 mm x 6 mm (46 095): narrow and
deep, just before the budget depth drops. The overall best is wide and shallow.

### 2.2 The 18 mm tool, for contrast (chip 0.1791 mm, ceiling 24000)

| | Width | Depth | Feed | Removal | Limits |
|---|---|---|---|---|---|
| idx 1 today | 5.5 | 10.0 | 9123 | 501 766 | none binds |
| idx 1 Pocket rule | 15.36 (0.854 D) | 2.35 | 8598 | 309 796 | force budget, corner coverage |
| idx 14 today | 3.6 | 4.17 | 10397 | 155 937 | none binds |
| idx 14 Pocket rule | 15.36 | 2.35 | 8598 | 309 796 | force budget, corner coverage |

At ceiling 18000 the Pocket pick is the same point at feed 6449: 232 347 mm3/min.
For the 18 mm tool the 3D Adaptive model does have a width limit at 10 mm depth: power. The
power ceiling crosses 10 mm between 7.2 mm and 9.0 mm of width.

### 2.3 A finding beside the question

The force budget (35.3 N, the hobby machine class) does not apply to the Adaptive family. By
the Pocket rule both stated roughs are outside it: the 6 mm cut at 1.2 x 6 mm by 56 % in
depth, and the 18 mm cut at 5.5 x 10 mm by a factor of about 4.

## 3. Does a vendor row print a stepover for this tool?

No. Two rows for the 6 mm tool in hardwood carry a width range, and neither is printed:

- `amana-flat-hardwood-adaptive-6000-2f`: 0.35 to 0.95 mm, "6% to 16%D".
- `amana-flat-hardwood-pocket-6000-2f`: 1.0 to 2.2 mm, "17% to 37%D".

Both are `derived`, grade c. Their notes say: "Not printed on the cited chart; repo-authored
band/ap range. The Amana Spektra Spiral Plunge v24 chart ... prints no range, ... and no ap
or ae range." The stored Spektra text has no stepover line. Printed radial values exist only
for aluminium (Garr: "Profiling Radial = .5xD"; Helical: "HEM RDOC = 0.200 in = 40% x D") and
for the Amana facing bits ("35% to 72%D").

## 4. The swept width, and the corner rule on an adaptive path

A geodesic 3D Adaptive holds the pass ceiling `c D`: `alpha = acos(1 - s/R) x 1.05`,
`c = (1 - cos alpha) / 2` (the rule quoted in `acceptance/wanaka_21.md`, idx 2).

| Stepover s (D 6) | c | Swept width c D |
|---|---|---|
| 1.2 (today) | 0.219 | 1.31 mm |
| 1.5 | 0.273 | 1.64 mm |
| 5.12 (the Pocket pick) | 0.893 | 5.36 mm |
| 6.0 | about 1.0 | about 6 mm (a full slot) |

The corner-coverage bound has no meaning for an adaptive path. It is a rule for offset rings:
two rings more than 0.854 D apart leave an uncut corner. An adaptive path clears by
engagement, not by ring spacing, so it leaves no such corner. Without that bound and without
the force budget, the rule finds no width limit on the 6 mm tool, and the best removal is at
the full diameter: a slot. That is not an adaptive cut.

## 5. The smallest change, its blast radius, and predictions

The change, one definition:

1. `computed_width` is true for the Adaptive family too (roughing, a stated-width radial).
   The corner-coverage limit stays on the Pocket family only.
2. The Adaptive family reads the force budget for its depth (`DepthCapRule::ForceBudget`),
   as every other roughing family does. The 2 x D factor stays as a second ceiling or goes.
3. One width ceiling for an adaptive cut. This needs a ruling; the repo has no printed value.
   Options: (a) `adaptive_woc_factor x D` of the machine (0.2 D in this project, 0.25 D by
   default; unsourced, today's add-door value); (b) 0.5 D, the width where chip thinning ends
   (a geometric fact: the lift is 1 from there); (c) no ceiling but D.
4. The force is read at the swept width `c D`, as the power case already is (P23a).

Predictions for the two wanaka ops on the 6 mm tool, ceiling 24000, from the table of 2.1
(Pocket feeds; the adaptive lift differs by a few percent, so these are estimates until a run):

| Width ceiling | idx 1 (stated 6 mm) | idx 14 (stated 3 mm) |
|---|---|---|
| (a) 0.2 D, stepover unset | about 1.0 mm x 6 mm, near 47 000 mm3/min; or 1.2 x 3.85 = 35 000 | 1.2 x 3.0: 27 000 (today's cut) |
| (b) 0.5 D | 3.0 x 3.09: about 56 500 | 3.0 x 3.0: about 54 900 |
| (c) D | 6.0 x 3.0 (a slot): about 109 700 | the same |
| With the stated stepover 1.2 kept (K2: never raised) | depth lowered 6.0 -> about 3.85, or width lowered to about 1.0 at 6 mm | no change |

Blast radius:

- FM1: 60 cells ship an Adaptive or 3D Adaptive value (30 each). 12 are plywood (no force
  line: no change). Up to 48 cells move in `proposal_ae_mm`, `proposal_ap_mm`,
  `binding_limit`, and `stepover_mm` and `depth_per_pass_mm` where the add door takes the
  proposal. The speeds columns move where the width moves.
- Pins: `the_pocket_proposal_is_the_removal_rate_maximum_p13c` (its "Adaptive keeps propose"
  arm), `the_add_door_stepover_is_the_proposal_p13c`, `the_roughing_depth_meets_the_force_budget_p13c`,
  `the_envelope_keeps_each_limit_k2b`, the F201 rows of `adaptive_doc_factor` and
  `adaptive_woc_factor`, the op-golden rows of Adaptive ops that take the add-door value, and
  the acceptance table rows of idx 1, 2, 14 and 15.
- The post-simulation depth gate (`rigidity_depth_cap`) then judges adaptive samples by the
  budget too.

## 6. Open points for the lead

- The Pocket answer for the 6 mm tool is "wide and shallow" (5.12 x 3.0 mm). The dial-in cut
  is "narrow and deep" (1.2 x 6.0 mm), and it is outside the force budget. One of the two
  must be the rule for an adaptive rough.
- The width ceiling of an adaptive cut has no source in the repo.
- The numbers of section 5 are estimates from Pocket feeds. A prototype run gives the exact
  values once the ruling on the width ceiling exists.
