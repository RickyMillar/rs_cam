# P11 step 0: preregistration (written before the run)

Author: agent p11-step0, 2026-11-04. This file was written before `model.py`
computed any model number. It is not changed after the run. A later analysis
is labelled "post hoc" in `REPORT.md` and on the page.

Disclosure: the author read the row values (chip band, ap and ae text) while
finding which rows state a depth and a width. No model output was computed
before this file was complete.

## Question

Does one model, radial chip thinning plus a helix engagement force model,
reproduce the vendor rows in `crates/rs_cam_core/data/vendor_lut/observations/`
with no constant tuned per row?

## The data the test can use

The rows give a chip band (advance per tooth, `CREDITS.md:330-358`). Most rows
give no feed: the chip is the printed quantity. Where `source_page` prints
`a" - b" IPM` and the row has `rpm_nominal`, the script also computes
`feed / (rpm x z)` and compares it with the stored band (a data check only).

Only two vendor families print a radial width: Garr and Helical (aluminium)
(`CHIPLOAD_LITERATURE_VERDICT.md` section 2.3 and N-8). Every other `ae` value
is repo-authored. The script treats an `ae` as printed only on Garr and
Helical rows.

A model can only be tested where the same vendor prints two conditions for
the same tool, so that one condition predicts the other. Two groups exist.

### Group W (printed width)

- Garr aluminium chart, per tool series and diameter: the slot row is the
  reference (ap 0.5 x D, ae 1 x D, the printed "Axial = .5xD (slotting)").
  It predicts the profile row (ap 1 x D, ae 0.5 x D), the A3 HEM row
  (ap 2 x D, ae 30-40 % D; the case uses ae 35 % D, the midpoint of the
  printed range) and the A3 finish row (ae 2.5 % D).
- The Garr finish row prints "Axial = Max LOC" and no number. Force and
  deflection candidates are "not testable" on that case. The chip candidates
  are tested.
- Helical: the "Traditional" row (ap 1 x D, ae 50 % D) predicts the HEM row
  (ap 2 x D, ae 40 % D).
- The Garr 3 mm slot rows and the Garr "20 %" general rows have no partner
  on the same chart. They appear in the per-row table only.

### Group D (printed depth table)

Every `flat_end` row whose `ap_rule` prints the 1 / 2 / 3 x D table (Onsrud,
Freud, Amana: "2xD reduce 25 %; 3xD reduce 50 %"). The 1 x D row predicts
the chip at 2 x D and at 3 x D. Each row gives two cases. The printed band at
k x D is the row band times 0.75 (2 x D) or 0.50 (3 x D).

## Assumptions about the vendor condition (one statement each, for all rows)

- A1. A depth-table chart prints its chip for a slot: ae = 1 x D, at
  ap = 1 x D. No chart states a width (N-8). Sensitivity: ae 0.5 x D and
  0.2 x D are reported, but the verdict reads A1.
- A2. The depth step keeps the width: 2 x D and 3 x D are at the same ae as
  the 1 x D condition.
- A3. The chip column is the programmed advance per tooth fz, not a mean or
  a peak chip (`CREDITS.md:330-358`; verdict section 8 item 1).
- A4. The reference chip of a row is the midpoint of its printed band. The
  predicted band (from the band ends) is also reported, as a weaker
  "intersects" result. The verdict reads the midpoint.
- A5. The cutter is a straight flat end mill with Z straight helical edges.
  Climb or conventional does not change any candidate below (the engaged arc
  [0, psi] and [pi - psi, pi] give the same set of chips).

## The model and its constants

Engagement angle: psi = acos(1 - 2 ae / D) (`feeds/force.rs:57`;
`ae_claims.py`).

Chip thinning (Garr, `CREDITS.md:354-358`; `feeds/geometry.rs:17-31`):
peak chip h_pk = fz sin(psi) for psi < pi/2, else fz. RCTF = fz / h_pk.

Mean chip (`feeds/operating_point.rs:242`; `material/force_line.rs` header):
h_m = fz (1 - cos psi) / psi.

Force line (ruling B6): tangential force per mm of axial edge
dF = Ks h + F_edge. Every candidate depends only on the ratio
c = F_edge / Ks (Ks cancels in a ratio):

| Material | c (mm) | Source |
|---|---|---|
| solid wood (hardwood, softwood) | 0.0060333 / 0.0768333 | `force_line.rs:265-266`, `CREDITS.md:433` (Curti 2021; both scale with rho, so c is density-free, `force_line.rs:21-22`) |
| MDF | 3.36 / 31.44 | `force_line.rs:90-94` (Goli 2018) |
| plywood | solid-wood c | Assumption. The repo line refuses plywood (`material/mod.rs:1318`). Unsourced; sensitivity c = 0 and MDF c |
| HDF, particleboard | MDF c | Assumption. The repo refuses both (`material/mod.rs:1321-1322`). Unsourced; sensitivity c = 0 and solid-wood c |
| aluminium, plastics, fibreglass | 0 (shear only) | Unsourced. The repo has no line (`material/mod.rs:1324-1325`). Sensitivity: solid-wood c |

Helix wrap: the edge lags by z tan(beta) / r radians at height z
(`tool/mod.rs:250-255`). Vendor helix angles are not in the rows. beta = 30
degrees is the rivmap100 tool (`planning/fixtures/rivmap100/rivmap100.toml:71`)
and is Unsourced for the vendor tools. Sensitivity: 0, 15, 45 degrees.

Engaged-edge force at rotation theta: sum over the Z edges and over z in
[0, ap] of dF(phi) for each edge element whose angle phi lies in the engaged
arc [0, psi], with h = fz sin(phi). The script integrates numerically (400
axial slices, 720 rotation steps over one flute pitch) and checks the mean
against the closed form ap Z / (2 pi) (Ks fz (1 - cos psi) + F_edge psi).

Deflection: uniform cantilever, stickout L, point load P at a = L - ap/2
(the repo form, `tool/mod.rs:749-770`); tip deflection is proportional to
P a^2 (3L - a). L is Unsourced. L = 3 x D (the least stickout that can cut
the printed 3 x D). Sensitivity: L = 4.5 x D, 6 x D, 7.5 x D.

## The candidates (fixed now)

For each case, a candidate holds one quantity equal between the reference
condition (at the reference chip) and the target condition, and solves for
the target chip fz2 (bisection; a candidate quantity rises with fz). If no
positive fz2 holds it, the prediction is 0 and the case fails.

| id | held quantity |
|---|---|
| I1 | peak chip h_pk (radial chip thinning only) |
| I2 | mean chip h_m |
| I3 | peak engaged-edge force, scalar sum of tangential forces (torque / r), max over theta |
| I4 | mean engaged-edge force (scalar, = mean torque / r) |
| I3v | peak tangential force resultant, vector sum (K_r = 0, because no wood K_r is sourced) |
| I5 | tip deflection from the I3v peak at a = L - ap/2 |
| C1 | chip first with a force cap: fz2 = min(I1 chip, I3 chip) |
| C2 | chip first with a mean-force cap: fz2 = min(I1 chip, I4 chip) |

C1 and C2 are the P11 flow: hold the peak chip, and lower it only where the
force would rise above the reference force.

## Pass rule

- A case passes for a candidate when the predicted fz2 lies inside the
  printed band of the target. A band with one printed value (min = max, or
  no min) passes within +/- 5 % of that value. The 5 % is Unsourced; the
  result at +/- 2 % and +/- 10 % is reported.
- A candidate reproduces the rows when it passes at least 80 % of the
  testable cases in group W AND at least 80 % of the cases in group D.
  The two groups are scored separately, because group D has many more
  cases.
- Verdict PASS: at least one candidate reproduces the rows with the
  primary constants (A1, beta 30, L 3 x D, the c table). Otherwise FAIL.
- A strict secondary result is also reported (not the verdict): the
  predicted ratio fz2 / fz1 within +/- 0.10 of the printed ratio
  (0.75 and 0.50 in group D; the band-midpoint ratio in group W).
- Eight candidates are tested. One pass among eight can occur by chance on
  group W (7 cases); the report states this if it happens.

## The lead's hand check (stated before the run)

Claim: in a slot, holding the force constant from 1 x D to 2 x D gives
about an 80 % chip cut; the vendors print 25 %. The script computes the cut
for the one-tooth peak model (`feeds/force.rs` form, ap (Ks fz + F_edge)),
for I3, I3v and I4, at fz = 0.131 mm (the rivmap100 chip,
`research_ae/REPORT.md` section 6(a)) in white oak, and over every group D
row. The claim is confirmed if the one-tooth cut at fz 0.131 is in
75-85 %.

## A failure names the missing term

If a candidate fails, the report names the quantity the rows imply and the
model lacks (for example: the force ratio the 25 % step implies, a width the
chart did not state, a heat or chip evacuation term).
