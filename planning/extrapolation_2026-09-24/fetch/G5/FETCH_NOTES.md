# G5 fetch notes: engaged geometry (V-bit width, tapered cone, bull corner)

Date: 2026-09-24. Phase 1 (fetch) of the extrapolation programme. No LUT,
manifest or Rust file is changed. Everything a reader needs to check a
number is in `sources/` (text) and `pdf/` (downloads, not committed), with
hashes in `sources.json`.

Files:

- `sources.json`: 24 source entries (URL, stored text, sha256, notes).
- `candidate_rows.json`: 171 candidate V-bit rows, written by
  `scripts/g5_candidate_rows.py`. The script finds each `verbatim` line in
  the stored text and stops if it is not there.
- `lut_discrepancies.md`: 8 LUT V-bit rows that are not on their cited
  document (D1, D2), and two minor encoding points (D3, D4).
- `scripts/g5_sources.py` writes `sources.json`;
  `scripts/g5_html_to_text.py` converts an HTML page to the stored text.

## 1. Answers per target

The marks used below:

- **printed**: the words or numbers are in the stored text.
- **derived**: this agent computed or inferred it. It is not a vendor claim.

### Target 1. Which diameter a V-bit chipload applies to

**Onsrud keys the V-bit chipload on the cutting diameter, the top of the V.**
(printed) All five Onsrud wood sheets (hard wood, soft wood, MDF, hard
plywood, soft plywood) print the 37-series engraving and V-bottom tools in
the table "Recommended Chip Load per Tooth by Cutting Diameter (in)":

| Series (catalogue PCT-19) | Cut | Printed cells (in/tooth, same on all five sheets) |
|---|---|---|
| 37-00 (60°, 1 flute) / 37-20 (30°, 1 flute), 1/4 in shank, tips 0.005-0.090 in | Varies | 1/4: .004-.006 |
| 37-50, 90°, 2 flute, solid carbide | 1/2 CED (hard wood, MDF) / 1/2 x D (soft wood, plywoods) | 3/16, 1/4, 3/8: .003-.006 |
| 37-60, 90°, 2 flute, carbide tipped | same | 3/8: .004-.006; 1/2: .004-.006; 3/4: .006-.008; 1: .008-.010 |
| 37-80 lettering bits, 2 flute, 60°/90°/120°/140° | Varies | 1: .004-.006; 1 1/4: .004-.006 (16,000 RPM); 2: .004-.006 (15,000 RPM) |

- The column heading is the cutting diameter. For 37-00/37-20 the value sits
  in the 1/4 column, the shank and body size, not the tip (0.005-0.090 in).
  (printed: catalogue p. 20 and the sheet column, checked on a rendered image)
- "Cut: 1/2 CED" on a 90° V bottom is the depth at which the engaged width
  equals the cutting diameter: w = 2 · ap · tan(45°) = D at ap = D/2.
  (derived) So the printed 37-50/37-60 chipload is the value at full
  engagement of the cone.
- The 37-60 chipload rises with the cutting diameter: band midpoints .005,
  .005, .007, .009 in at 3/8, 1/2, 3/4, 1 in. (derived from printed cells)
- The sheet header "DEPTH OF CUT: 1 x D ... 2 x D reduce 25 % ... 3 x D
  reduce 50 %" names D without saying which D. The only D in the table is
  the cutting diameter. (derived reading)
- Two catalogue mismatches (printed both sides): the sheets print a 37-60
  cell at 3/8 in, but the catalogue lists 37-60 at 1/2, 3/4, 1 in only. The
  sheets print a 37-80 cell at 1 1/4 in, but the catalogue lists 1, 1-1/2
  and 2 in. The rows transcribe the sheets.

**Amana keys the V-bit chipload on the angle and the tool number only.**
(printed) The AMS-159, Spektra engraving, 2-flute 15/60/90 engraving and
insert V-groove charts print one chip load per angle for every tip width and
every tool size. Three of them print "Depth of Cut: 1 x Tool Diameter" and
"To find RPM: (SFM x 3.82) / diameter of tool". The insert chart prints no
depth at all. No Amana chart prints a diameter column, an engaged width or a
depth-dependent value.

**PreciseBits prints the V width formula and the depth per pass, not a
chipload.** (printed)

- Calculator: "(TAN(Half Angle) × 2 × Cutting Depth) + (Runout + Tip
  Diameter) = Cutting Width" (`precisebits_calc.txt`; the same formula is in
  `calcv10.js`).
- EM2E4 1/4 in V-tip (0.015 in tip): stepdown Hardwood 0.118 in (3.0mm),
  Softwood 0.250 in (6.4mm), MDF "0.188 in. (3.0mm)" (the page prints this
  mm value; 0.188 in is 4.8 mm), Plastic 0.118 in. Feedrate: "Depends on the
  material being cut. Do the Sweetspot Test".
- EM2E8 micro V-tip (0.005 in tip): stepdown Hardwood 0.020 in, Softwood
  0.030 in, MDF 0.020 in.

**Vectric** (grade c) computes chip load from flutes, RPM and feed only. The
V-bit "Diameter" is the tool's own diameter. **CNCCookbook** (grade c, a
software vendor's blog) says the diameter in the cut is not the tool
diameter ("The effective diameter in an 0.010" depth of cut is only
0.020"") and uses the effective diameter for surface speed and RPM, not for
a printed chipload.

**Whiteside** prints RPM only on its V-groove pages.

**Result for target 1:** Onsrud, the only chart that states a diameter, keys
the chip load on the cutting (top) diameter, at a depth that engages the
full cone on its 90° tools. Amana states no diameter. No vendor I could
reach prints a chip load at an engaged width, or a scale from the top
diameter to the engaged width. The engine today looks a V-bit up at its
nominal diameter (`feed_ladder_diameter_mm`, `depth_cap_diameter_mm`) and
de-rates the band at the engaged width (the doc comment of
`feed_ladder_diameter_mm`). The Onsrud rows support the first choice. No
printed row supports the second. (derived comparison)

### Target 2. Tapered ball on the engaged diameter

- PreciseBits CM204/CM304 tapered ball nose (printed): "Stepdown (pass
  depth, depth per pass) recommended - 1X tip dia. / maximum - 2X tip dia.",
  stepover "0.08 X tip diameter (8%)" finish and "0.40 X tip diameter
  (40%)" roughing, "Feedrate (IPM) = Depends on the material being cut. Do
  the Sweetspot Test". No chip load. EVIDENCE 5.1-8 and 3.5-8 quote this
  page correctly.
- The Onsrud 77-100 tapered rows (already in the LUT) key on "cutting
  diameter", which the LUT note reads as the tip. I found no page that says
  more than that.
- The PreciseBits sweet-spot tutorial gives a test start feed "F = 0.03 x D
  x No. flutes x RPM (3% chipload per flute)" for woods, where D is "the
  diameter of the bit being tested". It is a test procedure for straight
  bits, not a recommendation, and not keyed on a tapered cone. (printed; not
  a candidate row)

**Result for target 2:** no chart keyed on the engaged diameter of a tapered
ball. The only printed tapered rule keys the DEPTH per pass on the TIP
diameter (1x recommended, 2x maximum). Not published anywhere I could reach.

### Target 3. Bull nose: full diameter or effective diameter

- PreciseBits MM208 bull-nose page (printed): corner radius 0.011 and
  0.023 in, "smooth bottom surface at higher chiploads than ball-nose
  cutters". No chip load, no effective-diameter rule.
- The Onsrud catalogue and the five wood sheets print no corner-radius
  router chip load.
- Harvey Performance "In the Loupe" (search results only, metal) says
  corner radius tools thin the chip along the radius. It prints no formula.
- A web search result (industrialmonitordirect.com, a blog) says "Treat the
  minor diameter (nominal OD minus 2x corner radius) as the effective
  cutting diameter". It is grade c at best, it contradicts the engine's
  ball-of-the-corner model, and I did not store it.
- The four LUT bull rows (`onsrud-bull-*`, `amana_3d_profiling.json`) are
  already `derived c` and cite no printed bull chart (R5, EVIDENCE 3.3-16,
  3.3-17).

**Result for target 3:** no wood vendor keys a bull-nose chip load on the
full diameter or on an effective diameter. Not published anywhere I could
reach. The radial thinning formula has a second witness (below), but it is
a radial rule, not an axial corner rule.

### The chip-thinning formula (second witness, radial only)

`calcv10.js` (PreciseBits calculator code, printed as code):
corrected chip = chip × D / (2 · sqrt(D · a − a²)) − runout. This is the same
algebra as `feeds::geometry::radial_chip_thinning_factor`
(1 / sqrt(1 − (1 − 2 a/D)²) = D / (2 sqrt(a (D − a)))). (derived
equivalence) It says nothing about V-bit depth, tapered cones or corner
radii.

## 2. Candidate rows (`candidate_rows.json`)

| Source | Rows | Grade | Notes |
|---|---|---|---|
| amana_insert_v_groove_v16 | 105 | a (hardwood, softwood, MDF); b (plywood, and the RC-1146/RC-1101 cells) | one row per (angle, flutes, RPM, material); no diameter_mm; single values as chipload_max only |
| onsrud_*_cutting_data (5 sheets) | 60 (12 each) | a | 37-00, 37-20, 37-50, 37-60, 37-80; diameter_mm = the printed cutting-diameter column |
| amana_15_60_90_vgroove_engraving_2f | 6 | b | 15/60/90° × softwood/hardwood; unit ambiguity (see below) |

Checks made by the script or by hand:

- Insert chart: every cell's feed/(RPM × flutes) agrees with the printed
  chip load within 25 %, except RC-1146 (120°, 1 flute, 14,000 RPM, 90 IPM)
  and RC-1101 (150°, 1 flute, 14,000 RPM, 90 IPM): 90/14000 = .0064 in
  against a printed .0024 in. Those rows are grade b. (derived check)
- 2-flute engraving chart: the heading says "Chip Load Per Tooth IPR**" and
  the footnote says "IPR** Inches per revolution". The printed feed, 50-125
  IPM at 18,000 RPM, gives 0.0028-0.0069 in per revolution, which matches
  the printed 0.003-0.007 in as a per-revolution value. Per tooth on two
  flutes it is half. Grade b. (derived check)
- The plywood rows from the insert chart are `derived b`: the chart prints
  one "Plywood/Chipboard" column; the split into `plywood_hardwood` and
  `plywood_softwood` is the row's mapping (the LUT precedent for Spektra
  "Wood/Plywood").
- `hardness_value` is absent on every row: no chart prints one.
- `operation_family trace`, `pass_role finish` follow the existing V-bit
  rows. No chart names an operation. The family placement is the
  reconciler's decision: the Onsrud 77-100 precedent puts one
  operation-less cell into every family the engine routes the tool to.
- The one "37-00/37-20" cell becomes two `exact a` rows (60° and 30°)
  because the sheet names both series. The one "Plywood/Chipboard" cell
  becomes two `derived b` rows because the chart does not name either
  plywood. The difference is on purpose.

Cross-group value (for the reconcilers, not a G5 claim):

- **G2:** the Onsrud 37-series rows print MDF, hard plywood and soft plywood
  for V-bits (printed values equal to the solid-wood values), and the insert
  chart prints MDF and Plywood/Chipboard. INVENTORY §1 counts 64 V-bit
  refusals in MDF/plywood for lack of such a row.
- **G1:** the 37-60 cells are a V-bit size series (3/8 to 1 in) on one
  sheet and one flute count. The 37-50 cells are flat across 3/16 to 3/8 in.
- **G2h:** the PreciseBits Janka calculator scales feed LINEARLY with the
  Janka ratio ("t = janka1 / janka2 × feed"); the repo uses a square root.
- **G4:** every Amana V-bit cell is a single value or a 0.003-0.007 in band;
  the Onsrud cells are bands (min/max 0.5 to 0.8).

## 3. What I searched

Web searches (WebSearch, 2026-09-24):

1. "V-bit chipload which diameter engaged width depth V-groove feeds speeds
   chart" → found the two Amana 2-flute engraving charts (new), AMS-159,
   Freud, ShopBot, CNCCookbook.
2. "Whiteside V-groove router bit chip load chart pdf" → no Whiteside chart;
   product pages print RPM only.
3. "Onsrud V-groove engraving tool cutting data chip load wood" → the
   Onsrud series pages and the catalogue.
4. "bull nose corner radius end mill chip thinning effective diameter
   shallow depth of cut chipload router wood" → blogs and calculators only;
   the PreciseBits MM208 page.
5. "Amana corner radius bull nose router bit speed chart chip load pdf" →
   ball nose, core box and 3D profiling charts; no bull-nose chart.
6. "Harvey Tool corner radius end mill effective cutting diameter shallow
   axial depth formula chip thinning" → metal blog posts, no formula.
7. "Vectric V-bit tool database feed rate chipload V-carve depth recommended
   stepdown help" → the Vectric help page (stored), forum threads (not
   stored).

Downloads (curl -L, 2026-09-24): the URLs in `sources.json`, plus:

- `https://onsrud.com/Series/EngravingTools.asp` and
  `https://onsrud.com/Series/37-{00,20,50,60,80}.asp`: dead end for
  geometry. The product tables load by script; the static HTML holds only
  the filter facets (cutting diameters). Not stored as sources; the
  catalogue gives the same data in print. The facets agree with the
  catalogue for 37-50 and 37-60 and list 1, 1.5 and 2 in for 37-80.
- `https://www.precisebits.com/scripts/calcv10.js`: stored as an excerpt.
- `https://www.precisebits.com/reference/precisefedsped.asp`: dead end. It
  says a feeds database will be published "as soon as we can" (Aug. 31,
  2009).

Earlier files in `pdf/` (`pb_jsall.1.02.js`, `pb_jsdefer.1.0.js`) hold no
calculator formulas; they are site scripts.

## 4. What I could not find

- A chip load at a V-bit's engaged width, or any vendor scale from the top
  diameter to the engaged width. Not published anywhere I could reach.
- A tapered-ball chip load keyed on the engaged cone diameter. Not
  published anywhere I could reach.
- A bull-nose chip load keyed on the full or the effective diameter for
  wood. Not published anywhere I could reach.
- A Whiteside V-groove chip load. Whiteside prints RPM only.
- The insert V-groove tools' diameters. The chart prints none; I did not
  pull the Amana catalogue.
