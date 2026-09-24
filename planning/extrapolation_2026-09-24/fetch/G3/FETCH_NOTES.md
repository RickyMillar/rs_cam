# G3 fetch notes: operation family and pass role

Date: 2026-09-24. Agent: G3 research (Phase 1). No cargo run, no LUT edit.

## Result in one paragraph

No vendor that I could reach prints two different chip loads for one tool in
two operation families or pass roles in wood. Every vendor that names a role
changes the **stepover** (or the depth) for that role. It does not change the
chip load. The one printed find is a wood chart for a **bull nose**
(Amana corner-radius spiral plunge, 1/4 in and 1/2 in). It prints no
operation, the same as the Onsrud 77-100 sheet. Its bands equal the Amana
ball-nose v7 bands at the same diameters.

## Files

- `sources.json`: 11 entries (7 stored here, 2 manifest charts re-hashed and
  stored, 2 cited from G5's stored copies).
- `candidate_rows.json`: 54 rows. 36 exact / grade a (Amana corner radius);
  18 derived / grade c (IDC V-bit). The generator is
  `scripts/g3_candidate_rows.py`. It checks every `verbatim` against the
  stored text.
- `lut_discrepancies.md`: the Phase 0 "two-role series" are derived rows;
  the four LUT bull rows are derived and a printed chart now exists.
- `pdf/`: downloads (not committed). `sources/`: stored texts.

## Per target

### Target 1: one tool, two roles (roughing and finishing) in wood

Not published anywhere I could reach.

- **Onsrud** (current sheets and the 2006 OC-06 catalogue). The sheets print
  an APPLICATION table that names tool series for Single Pass, Roughing and
  Finishing, and then one chip-load row per series. A series named under two
  applications still has one row. Example: 52-200/57-200 is "Single Pass
  GOOD" and "Roughing GOOD" on the current Hard Wood sheet, and "Finishing
  BETTER" on the 2006 Soft Wood sheet.
- **Derived, not printed:** Onsrud's finisher and rougher are different tools.
  At 3/8 in and 1/2 in on the current Hard Wood sheet, the 60-200 finisher
  prints .006-.008 and .007-.009. The 60-800 rougher prints .017-.019 and
  .019-.021. The ratio is about 0.4. The column alignment comes from
  `pdftotext -layout`; it agrees with the 2018 image sheet for the 77-100 row.
  This ratio does not transfer to one tool. The two tools differ in flute
  count and edge form (the OC-06 index names 60-200 "SC 3/E Low Helix
  Finisher" and 60-800 "SC D/E Roughers").
- **Amana ball nose v7, ZrN 2D/3D and Spektra 2D/3D charts.** One band per
  diameter and material. No role. The Spektra and ZrN charts title
  themselves "2D/3D Carving".
- **Freud** (`freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf`,
  already in the manifest without a hash). One band per diameter and
  material, based on cut depth = diameter. No role, no ball nose. Stored and
  hashed here (`sources/freud_cnc_20170822.txt`).
- **PreciseBits** tapered ball: "finishing pass (stepover, cleanup pass) =
  0.08 X tip diameter (8%)", "roughing pass (clearance stepover) = 0.40 X tip
  diameter (40%)". The feed comes from a test cut. Role = stepover.
- **IDC Woodcraft**: one feed and one RPM per bit. The ball nose note says
  "When using 1/8" ballnose for 2.5D & 3D relief carves, set stepover to
  5-8%". Role = stepover.
- **Sorotec** (German dealer): one fz per diameter and wood class, no tool
  type, no role.

### Target 2: bull nose (corner radius) in wood

Found: one printed chart, three copies.

- Amana 2 flute solid carbide spiral plunge with corner radius, 46460
  (1/16 R x 1/4 D) and 46462 (1/8 R x 1/2 D). Columns Soft Wood, Hard Wood,
  MDF, Plastics, Aluminum. 18,000 RPM, 1 x D. The ZrN (46460-Z) and
  Spektra (46460-K) charts print the same wood values.
- The file comes from Toolstoday's attachment store. The copyright line is
  Amana Tool. amanatool.com returned HTTP 403 to WebFetch and to curl for the
  HTML product page, so I did not see the chart on amanatool.com.
- **What it covers for the matrix.** The matrix queries bull nose at
  3.175 mm and 6.0 mm. 6.0 mm is inside 6 % of the printed 6.35 mm row.
  3.175 mm is a 2x size step below the smallest printed size; that is a G1
  question. Plywood has no column; plywood cells stay G2 refusals.
- **Derived, not printed:** at 1/4 in and 1/2 in, the corner-radius bands
  equal the Amana 2 flute ball-nose v7 bands for softwood, hardwood and MDF
  (1/4 in: .007-.009, .005-.007, .006-.008; 1/2 in: .009-.011, .007-.009,
  .008-.010). Amana gives a bull nose the same chip load as a ball nose of the
  same diameter.
- The chart names no operation. The 36 rows copy each printed cell into six
  engine families (parallel, trace and scallop as finish; contour, pocket and
  adaptive as roughing), as the LUT does for the Onsrud 77-100 rows. The 48
  bull refusals sit in parallel/finish (DropCutter, RampFinish, RadialFinish,
  HorizontalFinish) and trace/finish (Trace, ProjectCurve)
  (`compute/catalog/registry.rs`).
- Not found: I reached no bull-nose wood chart from Onsrud (current sheets and
  the 2006 catalogue print no corner-radius series) or Freud (chart read).
  Searches for Whiteside, Carbide 3D and Harvey bull-nose wood data returned
  no chart. The Amana
  Multi-Helix and Radius-Chamfer charts are AlTiN end mills for metal (search
  result titles); I did not fetch them.

### Target 3: V-bit clearing / pocketing / adaptive, and V-bit 3D finishing

Clearing: no printed chip load. Two sources state the role only as stepover.

- **IDC Woodcraft** (grade c, benchtop): one feed and one RPM per V-bit, a
  "Clear Pass Stepover" (20 % or 30 %) and a "Final Pass Stepover" (0.005 in
  or 0.02 in). The clear pass and the V-carve pass therefore run at the same
  feed and RPM. The 18 candidate rows carry the computed chip load
  (feed / (RPM x flutes); 0.033-0.035 mm for the 1/4 in 30/60/90 deg bits),
  row_kind derived, grade c. The starter-set table and the V-BITS table in the
  same PDF disagree for the 60 deg (40 vs 60 in/min) and 90 deg
  (50 vs 45 in/min) bits. The rows use the V-BITS table, which the metric
  section repeats.
- **PreciseBits** V-tip: "Stepover (for final cleanup pass) = 0.006 in." and
  "Stepover (for roughing, clearance pass) = 0.010 in."; the feed comes from a
  test cut.
- The IDC V-bit chip loads are about 2-5 times below the Amana V-groove trace
  rows already in the LUT (0.076-0.178 mm). IDC states benchtop machines.
  The reconciler should not mix the two without a machine-class ruling.

V-bit 3D finishing (waterline / steep-shallow): **not published anywhere I
could reach.** No vendor names a V-bit for 3D surface finishing.

### Target 4: tapered ball on profile / trace / waterline

Not published anywhere I could reach as a separate value.

- Onsrud 77-100: one row per diameter, no operation named.
- Amana ZrN and Spektra "2D/3D Carving" charts: one value for 2D and 3D work
  together. The title is the only statement that the value covers 2D work.
- PreciseBits tapered pages print no chip load at all (feed from a test cut).
  I did not fetch the Whiteside SC64/SC66 pages; the LUT holds them only as
  a Fusion 360 preset (grade c).
- Consequence: a contour/trace tapered-ball row would copy the same printed
  value into another family. That is a routing ruling for the operator, not
  new evidence. I emitted no such rows.

## Searches and URLs (including dead ends)

Web searches (WebSearch):

1. bull nose corner radius router bit wood chip load chart pdf
2. tapered ball nose 3D carving chip load roughing finishing hardwood chart
3. V-bit pocket clearing chip load wood recommended feeds V-carve chart
4. Amana Tool corner radius bull nose spiral CNC router bit speed chart wood
5. Carbide 3D feeds and speeds chart hardwood #201 #112 #301 V-bit Shapeoko docs
6. "ball nose" chip load "roughing" "finishing" wood router bit chart pdf manufacturer
7. amanatool.com productattachments corner radius speed chart pdf
8. toolstoday corner radius spiral plunge solid wood 46xxx Amana chip load IPM RPM
9. tapered ball nose speed chart wood pdf Amana 46xxx 3D carving "tapered" chip load
10. Whiteside tapered ball nose CNC router bit feed speed chart hardwood
11. Fräser Holz Schnittwerte Schruppen Schlichten Zahnvorschub fz Kugelfräser Tabelle pdf
12. router bit chip load chart wood "finishing pass" "roughing pass" ball nose vendor chart inch per tooth
13. bits and bits tapered ball nose recommended feeds speeds hardwood roughing finishing chipload
14. Onsrud corner radius bull nose series 3D wood router tool chip load
15. Whiteside bull nose corner radius spiral CNC bit feeds speeds wood RPM IPM
16. "3D finishing" "3D roughing" router bit wood recommended chip load table vendor bull nose ball nose
17. microfence Onsrud catalog pdf OC-06

Fetched:

| URL | Result |
|---|---|
| amanatool.com/products/.../solid-carbide-spiral-plunge-with-corner-radius-cnc-router-bits-for-solid-wood.html | 403 (WebFetch and curl) |
| toolstoday.com/v-14686-46462.html, v-15085-46460-z.html, v-15086-46460-k-bit.html | 200; gave the attachment names |
| toolstoday.com/content/ProductFile/Attachments/Solid-Carbide-Spiral-Plunge-w-Corner-Radius.pdf | 200, stored |
| …/ZrN-Coated-Solid-Carbide-Spiral-Plunge-w-Corner-Radius.pdf | 200, stored |
| …/Solid-Carbide-Spektra-Coated-Spiral-Plunge-w-Corner-Radius.pdf | 200, stored |
| amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf | 200, stored |
| community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf (IDC) | 200, stored |
| microfence.com/wp-content/uploads/2017/04/Onsrud.pdf | 200, same sha256 as the earlier-run file, stored |
| onsrud.com/images/Hard%20Wood.pdf | 200, sha256 equals the manifest; not stored again |
| freudtools.com/.../freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf | 200, stored; no role, no ball |
| sorotec.de/webshop/Datenblaetter/fraeser/schnittwerte.pdf | 200, stored (dead end for G3, useful for G1/G4) |
| stepcraft-systems.com/images/SC-Service/SC_Fraesparameter.pdf | 403 |
| shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf | 403 |

Files left in `pdf/` by an earlier run:

- `fablab_feeds.pdf`: an S3 "NoSuchBucket" error page. Dead end.
- `shopbot_feeds.pdf`: an HTML "429 Too Many Requests" page. Dead end.
- `onsrud_h_wood2.pdf`: a 2018 image-only Hard Wood sheet ("Cutting Data
  Sheets", sha256 `e3df6499…`). The earlier run did not record its URL, and
  no OCR tool is on this machine. I rendered it and read it. It has the
  same application table, no bull series, and the same 77-100 values as the
  LUT. It is not a source here, because it has no URL.
- `amana_ball_v7.pdf`: the manifest's chart; the hash matches.
- `microfence_onsrud.pdf`: the OC-06 catalogue (see above).

## What the reconciler needs from G3

1. The G3 trend in vendor data is **one chip load per tool, whatever the
   operation**. Role changes the stepover and the depth. Amana
   says so by title ("2D/3D Carving"). Onsrud says so by structure (one row per
   series under several applications). IDC and PreciseBits say so by the
   stepover columns.
2. So a G3 claim "copy the tool's printed row into another family at the same
   chip load" has vendor support as a structural fact. It has no support as a
   printed per-family number. The engine's own chip thinning and depth
   rules then act on the stepover and the depth.
3. The bull nose has a printed row set at 6.35 and 12.7 mm (softwood,
   hardwood, MDF). The 3.175 mm and plywood cells still need G1 and G2.
4. The LUT has zero printed series whose two roles carry distinct values (see
   `lut_discrepancies.md` §1).
5. **Treat the bull-nose rows and a tapered-ball contour/trace row alike.**
   The 36 bull rows copy a printed cell into six families. A tapered-ball
   contour or trace row would copy the Onsrud 77-100 cell the same way. Both
   charts print no operation, so both are the same act. I made it for the bull
   because no printed bull row existed at all. I withheld it for the tapered
   ball because its extra families are the G3 question itself. The reconciler
   should apply one rule to both. Either bundle both, or keep the bull rows to
   the four Onsrud-precedent families (parallel, scallop, pocket, adaptive)
   and let the G3 claim supply the rest. The 48 bull refusals need only
   parallel/finish and trace/finish.
6. The IDC rows use `tool_subfamily: idc_vbit`, a new string. If the lookup
   keys on the subfamily, the reconciler must pick or map it.
