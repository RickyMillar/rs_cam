# Extrapolation programme: the rulings (Phase 4)

Date: 2026-09-24. Status: **QUESTIONS, not ruled.** The recommendations are
the orchestrator's; the operator decides. No number moves before a ruling.

Evidence: `EXTRAPOLATION_G1.md` ... `EXTRAPOLATION_G9_machine_class.md`
(section 3 of each), `INVENTORY.md`. Verified rows wait in
`fetch/<G>/verified_rows.json`; none is in the LUT.

## The short version

The data changes the frame of the programme in three ways.

1. **Most gaps close by transcription, not by a law.** The fetch found
   printed rows for the acceptance case (a 1.0 mm tapered ball: Amana
   0.019-0.051 mm, SpeTool 0.0254 mm), for V-bits in MDF and plywood (76
   Onsrud rows), and for bull-nose cutters (Amana corner radius). Loading
   printed rows is not an extrapolation. It needs only the frame rulings
   below.
2. **No generic law survives.** The vendors disagree on the size exponent
   by 4x (0.29-0.49 against 0.75-1.25), on the sign of the MDF ratio, and
   on the band width per family. Every group recommends per-family claims
   or a refusal. The shipped 0.61 is between two vendor clusters and fits
   neither.
3. **Some shipping numbers have no source.** 42 ball-nose finish cells
   ship at 0.13-0.21 of the printed band (repo-derived rows). 26 MDF cells
   ship at x0.80 because two Janka tables disagree. The Shapeoko chart, the
   machine vendor's own, is about a quarter of the Amana figure the card
   ships for your machine.

## A. Frame rulings (they unblock several groups)

### A1. Tapered ball: which diameter is the lookup key? (G1-R4, G5 §3.4)

Every tapered chart (Onsrud, Amana, SpeTool, Whiteside) prints the chipload
against the **tip** diameter. The engine looks the row up at the **engaged
cone diameter** at the cut depth (R2, 2026-09-23). On a 7 deg taper that
scales a tip row by 0.70x-1.22x, a scale no chart prints.

- (a) Look up at the tip. Keep the cone diameter for the depth ladder and
  the deflection gate only.
- (b) Keep the cone key. The card shows the `(d / tip)^0.61` scale as an
  `Extrapolated` claim, valid for a depth of 0.5-2x tip (PreciseBits).

**Recommendation: (a).** It reads the chart as printed. The R2 rule that
Suggest and the gate use one diameter still holds for the depth ladder.

### A2. A single printed value: a top or a start? (G4 §3.1)

Amana Spektra prints one chipload per size (83 bandless cells). No text
says whether it is a maximum or a starting point. Freud, Amana and
ToolsToday call their figures "recommended starting points"; the one sheet
that prints a single value beside a band (AMS-159) puts it at the band's
low edge. That evidence leans to "start".

- (a) Top: the value is the maximum. Derived minimum = value - 0.002 in
  (the printed band width of the flat-end charts): 0.50-0.67x the value,
  inside R1 for all 75 flat-end cells.
- (b) Start: the value is the minimum. A derived band above it raises the
  breakage cap. That needs a second witness first.
- (c) Neither: keep one line, no band. Give the modulator and the advisor a
  "point" mode.

**Recommendation: (a) for Spektra flat end at 3 mm and above, labelled
derived on the card. Refuse a derived band below 3 mm and for V-bits.** It
keeps the printed value as the cap, which is the safe side, and closes the
advisor gap. Rule (b) waits for a second witness.

### A3. A printed value that names no operation: does it serve every family? (G3 §3.2)

The vendors print one value per tool. A pass role changes the stepover and
the depth, not the chip load (every same-tool pair: finish / rough = 1.00,
n = 20). The LUT copies such a row into each family, and the card does not
show the copy.

- (a) Copy rows into every family (today's Onsrud 77-100 shape). The card
  does not show the transfer.
- (b1) One row per printed cell. A G3 claim supplies the other families,
  and the card shows "vendor prints one value per tool; family transferred".

**Recommendation: (b1).** It follows the R1 record rule. It serves 58 of
the 126 G3 cells (bull nose 18, tapered ball 34, ball-nose MDF 6).

### A4. Hardwood from a chart column labelled "Wood" (G1-R1)

The micro tapered charts (Amana ZrN v8, SpeTool) print one "Wood, MDF,
Sign-Foam" column. The R5 convention (ruled) reads it as softwood and
hardwood, derived grade b. A 2026-05-02 manifest note refuses hardwood
under 1 mm. The acceptance case is hardwood.

**Recommendation: keep R5.** Hardwood reads the "Wood" column as derived
b. The card says "the chart prints 'Wood'; hardwood is not printed apart".
Without this, "3D Finish 6" stays refused.

## B. Per-group rulings

### B1. G1 size

- Load the verified micro rows (96, all confirmed), with the tapered rows
  filed as `tapered_ball_nose` (G1-R2: six LUT `amana-ball-*-zrn` rows file
  tapered tools as ball nose today).
- A size claim is **per source**: interpolate between the printed sizes of
  the anchor's own series. Outside the series span plus one step, refuse.
  Retire the generic 0.61 exponent.
- The size rule (`micro_extrapolation_refusal`) is replaced for tapered
  balls by "inside a chart's printed tips". It stays for ball, bull and
  V-bit, which have no micro series. Refuse a tip under 0.5 mm.
- **Acceptance:** "3D Finish 6" gets Amana 0.019-0.051 mm (2 flutes, 1.0
  mm tip), with SpeTool 0.0254 mm as a second vendor inside that band.

**Recommendation: yes to all four.**

### B2. G2 material category

- Load the 76 Onsrud 37-series V-bit rows (MDF, plywood, chipboard). This
  is transcription. Then run the `VBIT_MDF_PLY` judgement again.
- **Defect:** one Janka table for the row default and the query. The
  26 MDF cells lose the x0.80. Only particleboard has a sourced Janka.
- The hardwood-to-softwood transfer (x1.55) is above every printed ratio
  (1.00-1.50): cap it per family at the largest printed ratio.
- Ball nose in plywood (24 cells): refuse. The only option is a generic
  "plywood = hardwood row x 1.00 [0.79, 1.18]" with witnesses from other
  tool families.

**Recommendation: load, fix, cap, refuse.**

### B3. G3 family and role

- **The 42 ball-nose finish cells.** They ship at 0.13-0.21 of the printed
  band from repo-derived rows. The printed value is about 5x higher. This
  is the wanaka200 "3x to 7x low" finding. **Recommendation: run the
  simulation witness first** (a ball finish pass at the printed band on the
  wanaka fixture: the chip after thinning against the band and the rubbing
  floor). Then replace the derived rows.
- The derived `onsrud-bull-*` rows (0.30-0.37 of the printed Amana bull
  band, 7 cells): replace them with the printed Amana corner-radius rows.
- V-bit adaptive (16) and V-bit 3D finish (8): refuse.

### B4. G5 engaged geometry (V-bit)

- Key a V-bit row at its printed key (the cutting diameter or the angle),
  and name the key on the card.
- V-bit parallel finish (16 cells): refuse. At the engaged width the law
  lands under half of every figure and under the rubbing floor in 19 of 28.

**Recommendation: yes.**

### B5. G6 drill

- Delete `DRILL_CHIPLOAD_MULTIPLIER` (2.5). No figure supports it.
- A flat end mill plunge: axial chip = side chip / flutes, from the Amana
  Spektra "Ramp Down" column (3.175-6.0 mm, one witness). This conflicts
  with the plunge envelope (400 / 350 mm/min per mm), which is below every
  Amana figure. The envelope or the claim has to go.
- Ball, tapered ball and V-bit drill cells: refuse (48 cells).
- Real wood drills (Onsrud, Leitz, CMT; 3 vendors agree at 0.13-0.50
  mm/lip) need a drill tool kind first. Later.

**Recommendation: delete the 2.5; ship the flat-end 1/Z claim and move the
envelope to the printed figure; refuse the rest.** 16-20 of 80 cells ship.

### B6. G7 material physics (Kc)

- Solid wood: the Curti density law (printed, one lab, 287-1080 kg/m3).
- MDF: Goli 2018 (two measurements from one lab agree within 10 %).
- Plywood, particleboard, plastics: refuse a power figure (one read or
  none; HDPE's 40.0 is a yield stress, not a cutting force).
- The force anchor (`LIT_KS`, `LIT_FEDGE`) is an MDF fit applied to
  hardwood. It goes under either form.

**Recommendation: per-family, as above.** It is the largest code change of
the programme. It can land after G1-G3.

### B7. G8 long and small tools

- Replace the 0.88 / 0.75 long-tool share with the engine's deflection
  model. **Consequence:** the load target on 232 roughing cells goes up,
  and 182 of them (the 2D operations) have no deflection envelope today.
  The envelope must extend to the 2D operations first.
- Micro tools: the minimum-chip rule (from metal data) is a bound only. No
  generic claim.

**Recommendation: extend the envelope first, then retire the share.**

### B8. G9 machine class

The Carbide 3D Shapeoko 3 chart (withdrawn, 1/4 in cutters only) gives
about 0.23-0.35x the Amana figure. The same vendor's Nomad chart matches
Amana. So the machine, not the cutter, sets the Shapeoko number. Your Pro
XXL is stiffer than a Shapeoko 3, so the chart is a lower bound for it.

- (a) A typed `machine_class` on the machine profile and on the rows. On a
  hobby-gantry profile the machine vendor's row outranks the cutter
  vendor's row. On an industrial profile it never matches.
- (b) Do not load the rows. The dial (0.85) stays the one load margin.

**Recommendation: ask the operator.** (a) cuts the operator's 1/4 in feeds
to about a third. That is a large change on one withdrawn chart. The dial
was ruled as the one margin (R4 Q8).

## C. Order of work after the rulings

1. A1-A4 (frames), then B1 (the acceptance case) and B2 (transcription and
   the Janka defect).
2. B3 simulation witness, then the 42-cell change.
3. B4, B5.
4. B7, B6 (model changes).
5. B8 if ruled (a).

Each landing re-runs the FM1 matrix and commits the CSV.
