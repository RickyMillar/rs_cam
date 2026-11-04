# P11 step 0: validation study report

Author: agent p11-step0, 2026-11-04. The lead wrote this file from the agent's text (the
harness blocks a report file from a subagent). Pass rule: `PREREGISTRATION.md`. Page:
`index.html`. Claims: `python3 -I assert_claims.py` (21 claims).

## 1. Question

Does one model reproduce the vendor rows, with no constant tuned per row? The model is radial
chip thinning plus a helix engagement force model.

## 2. What the rows can test

- The 26 files hold 506 rows. 286 rows are flat end mills with a chip and a diameter.
- 84 rows print an IPM at an RPM. In all 84, feed / (rpm x z) is within 2.8 % of the stored
  chip (C16). The chip column is the advance per tooth.
- Only Garr and Helical print a width (18 rows, metal). The 20 other width values are
  repo-authored (C03, N-8).
- Group W: 7 cases. The Garr slot row predicts the profile, HEM and finish rows. The Helical
  traditional row predicts the HEM row.
- Group D: 466 cases from 233 rows that print the 1 / 2 / 3 x D table.
- The wood charts print no width. Assumption A1, for all rows: the wood chart chip is for a
  slot at 1 x D.

## 3. Model and constants

- The engagement angle is `psi = acos(1 - 2ae/D)` (`feeds/force.rs:57`).
- The peak chip is `fz sin psi` below 90 degrees (Garr; `feeds/geometry.rs:17-31`).
- The mean chip is `fz(1 - cos psi)/psi` (`feeds/operating_point.rs:240-242`).
- The force line is `dF = Ks h + F_edge` (ruling B6). A ratio test reads only `c = F_edge/Ks`:
  - wood: 0.0785 mm (`material/force_line.rs:265-266`)
  - MDF: 0.1069 mm (`force_line.rs:90-94`)
  - plywood, HDF and particleboard: a borrowed value (Unsourced)
  - metal and plastics: 0 (Unsourced)
- The engaged-edge force sums dF over the arc with the helix lag (`tool/mod.rs:250-255`).
- The helix is 30 degrees and the stickout is 3 x D. Both are Unsourced.
- The eight candidates hold, in order: the peak chip (I1), the mean chip (I2), the peak force
  (I3), the mean force (I4), the force resultant with K_r = 0 (I3v), the tip deflection (I5),
  min(I1, I3) (C1) and min(I1, I4) (C2).

## 4. The pass rule (preregistered)

- A case passes when the predicted chip is inside the printed band of the target row. A
  one-value band passes within +/- 5 % (Unsourced).
- A candidate passes when it passes at least 80 % of group W AND at least 80 % of group D.
  The two groups are scored separately.
- The verdict is PASS if one candidate passes with the primary constants (A1, helix 30
  degrees, stickout 3 x D, the c table).

## 5. The result

| candidate | group W | group D | group D, wood only |
|---|---|---|---|
| I1 chip thinning | 4 / 7 (57 %) | 4 / 466 (0.9 %) | 1 / 418 |
| I2 mean chip | 4 / 7 (57 %) | 4 / 466 (0.9 %) | 1 / 418 |
| I3 peak force | 1 / 6 (17 %) | 28 / 466 (6.0 %) | 0 / 418 |
| I4 mean force | 4 / 6 (67 %) | 6 / 466 (1.3 %) | 0 / 418 |
| I3v force resultant | 1 / 6 (17 %) | 34 / 466 (7.3 %) | 0 / 418 |
| I5 deflection | 1 / 6 (17 %) | 19 / 466 (4.1 %) | 16 / 418 |
| C1 chip first, peak cap | 1 / 6 (17 %) | 28 / 466 (6.0 %) | 0 / 418 |
| C2 chip first, mean cap | 4 / 6 (67 %) | 6 / 466 (1.3 %) | 0 / 418 |

- No candidate passes in any of the 13 sensitivity runs (C15). The runs change one value at
  a time: the helix (0, 15 and 45 degrees), the stickout (4.5, 6 and 7.5 x D), the tolerance
  (2 % and 10 %), the assumed width (0.5 and 0.2 x D) and the Unsourced c (zero, wood, MDF).
- The strict ratio test gives lower fractions still (C21). In this test, the predicted ratio
  must be within 0.10 of the printed ratio. No candidate reaches 20 % of group D or more than
  2 of 7 cases in group W.
- The Garr finish row prints "Axial = Max LOC". Thus the force candidates cannot test that
  case.
- The few force passes in group D are on aluminium and plastics. On those materials, c = 0 is
  Unsourced.

### Group W detail

- Garr slot to profile (242M, 842M, A3): chip thinning gives the slot chip, 0.06 mm. That chip
  is the floor of the profile band (0.06-0.12). The case passes on the band edge. The printed
  midpoint is 1.5 times higher, so the strict ratio test fails (C20).
- Garr A3 HEM (2 x D, 35 % D): chip thinning gives 0.063 mm. Garr prints 0.12-0.18 mm (C12).
  The chip from chip thinning is too low; the vendor chip is about 2 times higher.
- Garr A3 finish (2.5 % D): chip thinning gives 0.192 mm. Garr prints 0.06 mm, 3.2 times
  lower (C13). Garr does not apply its own thinning rule to its finish row.
- Helical traditional to HEM: chip thinning gives 0.252 mm (+7.3 %). The peak force I3 gives
  0.2245 mm (-4.5 %). Helical prints 0.235 mm (C14).

### The lead's hand check (slot, 1 x D to 2 x D)

- The one-tooth peak model (the `feeds/force.rs` form, ap (Ks fz + F_edge)) at fz 0.131 mm in
  white oak needs an 80.0 % chip cut to hold its force. The hand check is confirmed (C07).
- The engaged-edge forms need these cuts (C08):
  - the peak force (I3): 78.2 %
  - the mean force (I4): 97.1 %
  - the force resultant (I3v): 61.1 %
  - constant tip deflection at a stickout of 3 x D (I5): 11.2 %
- The vendors print 25 %. A pure shear line (c = 0) gives 50 %. The edge term makes the cut
  larger, not smaller (C09).
- There are 209 wood rows at 2 x D (A1 slot). Their median peak-force cut is 74.6 %, with a
  range of 48.1 % to 100 %. Their median mean-force cut is 92.6 % (C10).
- The vendor step lets the peak engaged-edge force rise. The median rise is 1.51 times at
  2 x D (range 1.26 to 1.83) and 1.72 times at 3 x D (C11). Thus the depth table does not
  hold the force, the chip or the peak chip.

## 6. Verdict against the pass rule

FAIL. The best value in group W is 67 % (I4, C2). The best value in group D is 7.3 % (I3v).
On wood, every force candidate except I5 passes 0 of 418 depth cases. One model of radial
chip thinning plus a helix engagement force does not reproduce the vendor rows.

## 7. The missing terms

1. The depth table (Onsrud, Freud, Amana) lets the force rise about 1.5 times at 2 x D. The
   model has no term for this step. The charts print no reason and no width (N-8).
   - Post hoc PH1: constant tip deflection at a stickout of 3.5 to 4 x D gives the 25 % step
     on six hardwood rows (C19).
   - No row prints a stickout. Thus that fit has one free constant, and it is not evidence.
2. Garr profile and HEM: the vendor chip rises as the time in cut falls. This rise is larger
   than chip thinning gives. The model has no heat or time-in-cut term.
   - Post hoc PH2: a chip in inverse proportion to psi / 2 pi fits 4 of 7 group W cases
     (C18). It fails Garr 142M, the Garr finish row and Helical. This idea came after the
     author saw the rows.
3. Garr finish: the vendor holds a low chip at 2.5 % D. This is a finish-quality term. A load
   model cannot give it.

## 8. Evidence for operator ruling D11 (may Suggest apply chip thinning against N-8?)

This section gives the evidence only. The operator makes the ruling.

- For wood, the rows cannot test chip thinning. No wood chart prints a width. Chip thinning
  changes the chip only through ae, and no wood row gives ae. The N-8 finding stands: the test
  is absent, not failed.
- In the two metal families that print a width, the direction of chip thinning agrees with
  the vendor between 35 % and 50 % D:
  - Garr HEM: the vendor chip is 2 to 3 times the slot chip. Chip thinning gives 1.05 times.
    The vendor goes further than chip thinning.
  - Helical: the vendor chip at 40 % D is 0.95 times the chip at 50 % D. Chip thinning gives
    1.02 times. The difference is 7.3 %.
- At a very small width, chip thinning disagrees with the vendor. At 2.5 % D, chip thinning
  gives 3.2 times the chip that Garr prints for its finish row. This is the only printed row
  below 35 % D.
- If Suggest applied chip thinning in wood, the result would rest on two items:
  - Garr's printed rule (`CREDITS.md:354-358`)
  - the metal rows between 35 % and 50 % D
  No wood row supports or refutes it. Between 35 % and 50 % D, chip thinning stays at or below
  the printed metal chip or within 7.3 % of it. Below 35 % D, the only printed row (Garr
  finish) disagrees.
- The depth table does not support a force-limited chip. If P11 holds the force (C1), the
  2 x D chip falls about 75 % in wood, where the vendors print 25 %.
  - A chip-first flow that keeps the vendor depth table as a separate printed limit (P11
    step 4) agrees with this evidence.
  - A flow that derives the depth step from the force model contradicts it.

## 9. Limits of this study

- Group W has 7 cases from two aluminium vendors, and eight candidates are tested. Thus one
  pass in group W could occur by chance.
- The wood result depends on A1 (a slot). At 0.5 D and 0.2 D the verdict is also FAIL.
- K_r is not sourced. Thus the force resultant (I3v) ignores the radial force.
- I4 uses the closed form of the mean force. The numeric sum agrees with it to 1 %
  (`model.py` checks this). This change was made after the first run. It moved one case on a
  band edge (Garr A3 slot to profile) to pass. It does not change the verdict.
