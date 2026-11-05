# P13 items 4 and 5, step 0: the vendor-row chip law and the industrial rows

Author: P13 items 4-5 agent, 2026-11-05. Read-only. HEAD `7048456f8`. The script
`analysis.py` reads every repo file at that hash (`git show`) and writes `results.json`.
`assert_claims.py` checks each claim id below: run
`python3 -I analysis.py && python3 -I assert_claims.py` (41 claims, all pass).
Each constant in `analysis.py` names its path:line, and the script asserts the line text.

## 0. Answer

1. A pooled law over the vendor rows (chip = a_class x D^b) misses a held-out vendor by a
   factor of 1.5 to 2.1 [B05]. The missing term is the chart series (the tool design and the
   machine class that the vendor assumes). One offset per series takes R2 from 0.555 to
   0.953 [B04, B12]. No vendor prints the reason for its level.
2. Inside one series the law is clear: chip ~ D^0.56, no flute-count term (Z^0.03), and no
   difference between softwood, hardwood and plywood. MDF is x1.38 [B12, B13].
3. Recommendation: candidate B, "the anchored series law". The cell reads the (Pocket,
   Roughing) row of its own tool and material, scaled by (D / D_row)^0.56. On all 32 FM1
   flat cells the row is at the cell's own D, so the chip is the roughing row x 1.00. The
   DropCutter route already does this today (G-DCFLAT) [A05, A10].
4. Op-family relation: x1.00. The one printed same-tool wood relation (Harvey 809500,
   Finishing = 0.76 x Slot-Rough) is a single witness at 0.1 D width and 3 x D depth. No
   printed source gives a value at the FM1 condition (depth at or under 1 x D; Trace is a
   full-width slot). Harvey does bear on P11b (section 5): it sits 9 % under the thinned
   reading and 54 % over the no-thinning reading [D01-D05].
5. Item 5: the four `industrial_only/` rows are ALREADY in the shipped lookup. Commit
   `51edd494d` (2026-06-01) put them into `observations/freud_solid_carbide.json` with the
   same ids and values. The `industrial_only/README.md` text "not loaded" is stale, and
   derate report M7 / [F03] ("rows absent") is wrong [C01-C03]. This conflicts with the
   operator intent "not a lookup to ship from". Lead decision needed (section 4).
6. The rpm half of the M1 double derate is a separate lead decision. Candidate B gives the
   6 mm hardwood Trace 2 694 mm/min at the SFM rpm (10 610) or 4 000 (the cap) at the chart
   rpm 18 000, against 973 today [E05].

## 1. The 60 formula-chip cells (step a)

The shipped formula and the SFM rpm reproduce every flat and ball cell [A03]: chip =
0.024 D^0.61 (J/600)^-0.504 (`machine/mod.rs:51-53`, `material/mod.rs:822,827`), rpm =
SFM x 1000 / (pi D) in 8 000-24 000 (`material/mod.rs:737-747`, `machine/mod.rs:936-937`).
A finish pass does not thin, so feed = rpm x chip x Z.

| Group | Cells | Ops | Why no vendor row applies |
|---|---|---|---|
| Flat end, 3.175 and 6 mm, 2F, 4 woods | 32 | Trace, RampFinish, RadialFinish, HorizontalFinish | The routed query is (Trace, Finish) or (Parallel, Finish). No embedded flat-end row sits in either family [A06]. DropCutter is also (Parallel, Finish) but routes `FlatParallelAsRoughPocket` to the Spektra row; the other three raster ops route `AsStated` (`registry.rs:879, 1309, 1424, 1485`) [A05]. |
| Ball nose, 3.175 and 6 mm, softwood and hardwood | 20 | Profile, Trace, Waterline, Pencil, SteepShallow | No ball row in the Contour or Trace family [A07]. `FAMILY_RULES` has no ball rule (`extrapolation/family.rs`, ruling B3 waits for its sim witness) [A09]. |
| V-bit, 6.35 and 12.7 mm, softwood | 8 | Face, Pocket, Profile, Zigzag | Every V-bit row is (Trace, Finish) [A08]. Profile reads the engaged diameter, not the nominal one. |

Flat cell values today (the four ops are equal per D and material):

| D | Material | Chip | Rpm | Feed |
|---|---|---|---|---|
| 3.175 | softwood / hardwood / MDF / plywood | 0.0486 / 0.0311 / 0.0358 / 0.0342 | 20 051 / 20 051 / 17 043 / 18 046 | 1 947 / 1 248 / 1 219 / 1 235 |
| 6 | softwood / hardwood / MDF / plywood | 0.0716 / 0.0459 / 0.0527 / 0.0505 | 10 610 / 10 610 / 9 019 / 9 549 | 1 519 / 973 / 951 / 964 |

Scope recommendation: item 4 covers the 32 flat cells. The 20 ball cells need the B3 family
rule (its sim witness is the open prerequisite), not a new law. The 8 V-bit cells have no
diameter series to fit (EXTRAPOLATION_G1 1.9: one V-bit pair, slope 0; ruling B4 keys a
V-bit row at its angle).

## 2. The fits (step b)

Data: the 26 embedded files, flat end, wood families, the `size.rs` `is_series_row` filter
(grade a or b, not fallback, a diameter and a chip). 209 rows. A copy of one printed cell
(the Spektra Wood/Plywood column under pocket and adaptive, softwood, hardwood and plywood)
counts once per material class: 165 points, Amana 73, Onsrud 80, Freud 12 [B01]. Log-log OLS,
weighted per point and per vendor (each vendor sums to 1). Full tables: `results.json`.

| Candidate | Form | R2 | rms factor | Leave one vendor out (printed / law, median) |
|---|---|---|---|---|
| C0 shipped formula | 0.024 D^0.61 (J/600)^-0.504 | < 0 | 3.82 | not fitted; printed / law 2.17 Amana, 4.95 Freud, 5.20 Onsrud [B03] |
| C1 per class | a_c D^b_c | 0.562 | 1.57 | Amana 0.47, Onsrud 2.21, Freud 1.53 |
| C2 shared slope | a_c D^0.717 | 0.555 | 1.58 | Amana 0.48, Onsrud 2.09, Freud 1.55 [B04, B05] |
| C3 + flutes | a_c D^b Z^c | 0.601 | 1.54 | Amana not testable (only Spektra prints 3F) |
| C4 Janka | a D^b Z^c (J/600)^h | 0.560 | 1.57 | h = -0.002 [B08] |
| diagnostic: + vendor offset | C3 + vendor | 0.801 | 1.36 | Freud x2.05, Onsrud x2.20 over Amana [B11] |
| diagnostic: + series offset | C3 + 37 series | 0.953 | 1.16 | D^0.560, Z^0.026 [B12] |

What each axis can carry:

- Diameter: inside a series the slope is 0.56. The vendors' own slopes differ: Amana 0.53,
  Freud 1.16; Onsrud 0.19 pooled over its series (its small and large sizes are different
  tool series) [B14]. EXTRAPOLATION_G1 1.9 found the same two clusters.
- Flutes: only the Spektra chart prints two flute counts [B02]. There 3F / 2F has a
  median of 1.00, range 0.63-1.25 (n 20) [B07]. The pooled Z^-1.10 of C3 is a vendor artefact: with a vendor
  offset it is Z^+0.06 [B06]. Chip per tooth does not depend on Z.
- Material: hardwood / softwood inside one series is 1.00 at Onsrud (n 11) and 0.86 at
  Freud (n 3) [B09]. The shipped formula puts hardwood at 0.64 of softwood. MDF / softwood
  disagrees in sign: Amana 1.25, Freud 1.16, Onsrud 0.85 [B10]. The Janka exponent fits to
  zero [B08].
- Missing term (D11): the series level. It is a factor of about 2 between Amana and the
  Onsrud and Freud charts at the same D [B11]. No row prints its cause. Candidates are the
  tool geometry (flute form, coating) and the machine class (Onsrud and Freud are industrial
  charts; the Spektra chart is also headed "INDUSTRIAL").

Pooled laws put a finish op above its own roughing row: at 6 mm hardwood C2 gives 0.170,
1.33 x the Spektra row 0.127 that the Pocket cell of the same tool reads [E04]. A trace pass
would then cut at a higher chip than the slot roughing pass. Thus no pooled law is
recommended as a shipped value.

Candidate B (recommended): chip = row x (D / D_row)^0.56, where the row is the (Pocket,
Roughing) row that the same tool and material read, and 0.56 is the within-series slope
(a computed exponent, not a typed one). The law carries the size; the series term stays in
the anchor row. On FM1 D = D_row, so B is the row x 1.00 [E04]. A tool with no (Pocket,
Roughing) row of its own material has no B value; it takes what the lead rules for the
formula path (refuse, or warn under D7).
Data note: the Freud chart prints one solid-carbide table "based on cut depth equal to bit
diameter" with no roughing or finishing split. The LUT labels its hardwood, softwood and
plywood rows `finish` and its MDF rows `roughing` [C07]. These labels are repo-authored. B
keys on (Pocket, Roughing), so a Freud anchor exists only in MDF until the labels are fixed.
Overlap, not in scope: the G1 form C typed exponent `CHIPLOAD_DIAMETER_EXPONENT` 0.61
(`vendor_lookup.rs:541`) serves the 88 `Extrapolated` cells; 0.56 would move them. That
is a separate decision.

## 3. The industrial rows as an out-of-sample baseline (step c)

The fits exclude the four ids, then predict them:

| Candidate | printed / law |
|---|---|
| C2 pooled (point weight) | 1.24 to 1.99 [C04] |
| B anchored at Freud's own largest smaller row | 1.11 to 1.76 [C05] |

Both laws are low. Freud's own slope (1.16) is twice the within-series 0.56, so the size
term also varies by series. The plywood row (anchored at 6.35 mm) misses most (1.76).

## 4. Item 5: the rows already ship

[C01-C03]. Options for the lead:

1. Delete the four `freud-solid-carbide-half-*` rows from `observations/` and keep the
   `industrial_only/` file as the only copy. A test loads it as a baseline (for example,
   "the anchored law is within a stated factor of each industrial row"). This matches the
   operator intent. No FM1 cell reads a Freud row [C06], so no FM1 cell reads these rows
   directly. A G1 form A or B claim on a Freud series could still move (the series gets
   shorter); the FM1 run gives the exact count.
2. Keep them shipped, delete the stale `industrial_only/` copy and correct the README and
   derate report M7.

## 5. What a finish or trace op reads (step d)

- Trace is a `Radial::Slot` op (width D): its engagement is the chart condition (slot,
  depth at or below 1 x D). The roughing-row chip applies with no change.
- Ramp, Radial and Horizontal are `PassRole::Finish`: P11b ships no chip thinning. They read
  the same chip.
- The only printed same-tool finish relation in the repo:
  `planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt:13,17`
  (softer), `:20,24` (harder), `:26,28` (engineered), `:30,32` (phenolic). Finishing /
  Slot-Rough = 0.763, 0.773, 0.775, 0.771 (median over 15 sizes; range 0.667-0.833)
  [D01, D02]. The finishing row is at 0.1 x D radial and 3 x D axial (`:17`) [D04].
- Model readings at (0.1 D, 3 x D), with the printed 3 x D table 0.5:
  - with Garr thinning (lift 1.667): 0.833. Harvey is 9 % below it [D03].
  - with no thinning (P11b for a Finish op): 0.5. Harvey is 1.54 x above it [D05].
  This one row leans toward thinning at a light width. That bears on the P11b rule "a finish
  pass does not thin". It is a single witness (a metal-tool maker's wood chart, not in the
  LUT). The lead decides whether it reopens P11b; the Garr finish row (metal, 3.2 x below
  thinning, P11 step 0 section 7.3) points the other way.
- Recommendation for item 4: x1.00 (the roughing-law chip; the P11b thinning rule as it
  stands). The reason: no printed source gives a value at the FM1 condition (depth at or
  under 1 x D, where the depth term is 1.0; Trace is a full-width slot). Harvey prints only
  the 3 x D finishing case.

## 6. Cells moved (step e)

Feed = floor(min(rpm x chip x Z, 4 000)); 4 000 = min(travel 4 000, default cap 6 000) of
`MachineProfile::default()` (`machine/mod.rs:941, 834`). The power ladder (0.8 kW) is not
modelled here; the FM1 re-bless gives the exact numbers.

- Every candidate moves all 32 flat cells by more than 5 % [E01]; no ball or V-bit cell moves.
- Candidate B at the SFM rpm: feed x1.77 to x3.28, 28 cells at x2 or more, 12 at the cap
  [E02]. At 18 000 rpm: 20 cells at the cap [E03].
- The pooled candidates: x1.9 to x4.2, 20 to 32 cells at the cap (`results.json` `moves`).

## 7. Lead decisions

1. Law: B (anchored series law, slope 0.56) or a pooled law (C2).
2. Rpm: keep the SFM rpm (Shapeoko port, Unsourced) or read the anchor row's rpm (18 000).
3. Op-family relation: x1.00 (recommended) or Harvey 0.76.
4. Item 5: option 1 (unship, test baseline) or option 2 (keep shipped).
5. Scope: ball cells through B3, V-bit cells left on the formula or refused.
