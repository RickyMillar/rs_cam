# G9: Machine class (the machine vendor's own chart)

Status: fetch and transcription done 2026-09-24. The 44 rows are parked in
`fetch/G9/candidate_rows.json` (not in the LUT, not in `EMBEDDED_FILES`)
until the Phase 4 rulings. The lookup does not read machine class yet
(§6). No cargo command ran for this package. The orchestrator builds and tests.

Artefacts: `fetch/G9/pdf/` (the two chart PDFs), `fetch/G9/sources/` (the
pdftotext output and the forum announcement text). Parked 2026-09-24 by the
extrapolation session until the Phase 4 rulings: the 44 rows are
`fetch/G9/candidate_rows.json` (they were `data/vendor_lut/observations/carbide3d_shapeoko.json`),
the manifest entry is `fetch/G9/sources.json`, and the stored chart text is
`fetch/G9/sources/carbide3d_s3_feeds_250.txt`. Nothing is in the live LUT.

## 0. The finding

Carbide 3D publishes **no current feeds chart**. The one official Shapeoko
chart is withdrawn:

- `https://docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf` returned
  200 in the Internet Archive captures from 2020-09-22 to 2021-11-18, with one
  digest. It returns 301 from the 2022-10-27 capture on. The live URL now
  redirects to `my.carbide3d.com`.
- The live `carbide3d.com`, `guides.carbide3d.com` and all 17 cutter product
  pages on `shop.carbide3d.com` print no feed, RPM or chipload.
- Carbide Create V8 (build 853, 2026-06-15) holds its tool library in
  `Contents/Resources/ccpro.db`. That file is encrypted (byte entropy 7.9998
  bits). It is not readable, and this package did not try to decrypt it.

The rows therefore come from the Internet Archive copy of the Shapeoko 3
chart. The PDF is an InDesign export, created 2019-07-30. The forum post that
announced the chart is dated 2016-07-14.

## 1. URLs read

| URL | Result | Prints numbers? |
|---|---|---|
| https://web.archive.org/web/20211118030340/https://docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf | 200, 582 504 B, sha256 `288543263d3fec1bb96ecdf0fadc292d46b50e650f20409c6de3d9975097501d` | **Yes.** The source of every LUT row. |
| https://web.archive.org/web/20221027042155/https://docs.carbide3d.com/support/supportfiles/Nomad883_feeds_125.pdf | 200, sha256 `e4cd0a15950886efea4849ead1877bcd3a2bda73a63c9213a8ad564cddb9aa32` | Yes. Nomad 883, a different machine class. Not transcribed (§4). |
| https://web.archive.org/web/20230328182355/https://docs.carbide3d.com/support/ | 200 | No. It links "Shapeoko Speed and Feed Chart" and "Nomad Speed and Feed Chart". |
| https://web.archive.org/cdx/search/cdx?url=docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf | capture history | No. It gives the 200 → 301 history in §0. |
| https://docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf, https://docs.carbide3d.com/support/, https://docs.carbide3d.com/tutorials/tutorial-feedsandspeeds/ | redirect to my.carbide3d.com home | No |
| https://docs.carbide3d.com/shapeoko/feedandspeed/, https://carbide3d.com/feeds-and-speeds/, https://carbide3d.com/hub/feeds-and-speeds/, https://my.carbide3d.com/feeds-and-speeds/, https://docs.carbide3d.com/resources/feeds-and-speeds/, https://docs.carbide3d.com/shapeoko-support/, https://carbide3d.com/hub/faq/feeds-and-speeds/ | 404 | No |
| https://carbide3d.com/sitemap.xml, https://my.carbide3d.com/sitemap.xml, https://guides.carbide3d.com/sitemap.xml | 200 | No. No feeds page is listed. |
| https://guides.carbide3d.com/create/tool-library/, https://carbide3d.com/hub/courses/create/tool-library/ | 200 | No. A video only; "Carbide Create comes equipped with multiple Tool Libraries for each machine as well as specific materials." |
| https://carbide3d.com/hub/courses/getting-started-cnc/tooling-introduction/ | 200 | No |
| https://carbide3d.com/blog/feed-and-speeds-part-1/, https://carbide3d.com/blog/feed-and-speeds-part-2/ | 200 | No. Guest posts (CNCCookbook, 2016). They point to "Carbide3D's new charts that are made and tested for their machines". |
| https://shop.carbide3d.com/collections/cutters and 17 product pages (101, 102, 102Z, 111, 112, 121, 122, 201, 201Z, 202, 251, 274Z, 278Z, 301, 302, 501, 502) | 200 | No feeds. They print flute counts: #201 3, #202 2, #251 2 (downcut), #274Z and #278Z 1, #301/#302 V-bits 90° / 60°, #501/#502 engravers 60° / 40°. |
| https://carbide3d.com/carbidecreate/download/, https://carbide-downloads.website-us-east-1.linodeobjects.com/builds.json, …/cc/stable/853/CarbideCreate-853.dmg (sha256 `35a06405…e80e`), …/CarbideCreate-853.exe (Inno Setup; not extracted) | 200 | No readable numbers (`ccpro.db` is encrypted). |
| https://community.carbide3d.com/raw/2402/1 (Carbide 3D staff, 2016-07-14) | 200 | No numbers. It states the test conditions: "#201 and #202 3 flute .25" endmill, 100% engagement", "These numbers will work for roughing and waterline cutting toolpaths". |
| https://community.carbide3d.com/uploads/default/original/2X/f/f3da60b5646c598f76f502ce98f81dcd4c64c6a3.pdf | 200 | Yes, but it is a user upload of an older chart revision (Dewalt / Makita dial columns, no RPM for most rows). Not a source. |

Correction to the task list: #301 and #302 are V-bits, not ball cutters. The
ball cutters are #101 (1/8"), #111 (1/16"), #121 (1/32") and #202 (1/4").

## 2. The printed rows

The chart header (an image, read from the rendered page) names two cutters
over one table: **#201 .25" Square** (3 flutes) and **#202 .25" Ball**
(2 flutes). The chart prints DOC, RPM, router dial, FEED (ipm) and PLUNGE
(ipm), one value per material. It prints no chipload, no stepover and no
range. Every LUT row is max-only (loader rule 1). The chipload is derived:
feed × 25.4 / (RPM × flutes).

### 2.1 Transcribed (44 LUT rows: 11 materials × 2 cutters × pocket and adaptive, roughing)

| Material (chart) | LUT family | DOC in (mm) | RPM | Feed ipm (mm/min) | Plunge ipm (mm/min) | #201 3F mm/tooth | #202 2F mm/tooth | Row kind |
|---|---|---|---|---|---|---|---|---|
| Pine | softwood | 0.4 (10.16) | 21 000 | 75 (1905) | 40 (1016) | 0.0302 | 0.0454 | exact, a |
| Mahogany | hardwood | 0.1 (2.54) | 18 950 | 65 (1651) | 32 (813) | 0.0290 | 0.0436 | exact, a |
| Plywood | plywood_softwood, plywood_hardwood | 0.25 (6.35) | 18 950 | 100 (2540) | 50 (1270) | 0.0447 | 0.0670 | derived, b (one row, two families) |
| MDF | mdf | 0.3 (7.62) | 17 000 | 80 (2032) | 30 (762) | 0.0398 | 0.0598 | exact, a |
| Acrylic | acrylic | 0.06 (1.524) | 19 000 | 65 (1651) | 25 (635) | 0.0290 | 0.0434 | exact, a |
| HDPE | hdpe | 0.125 (3.175) | 21 280 | 80 (2032) | 40 (1016) | 0.0318 | 0.0477 | exact, a |
| Delrin | delrin | 0.08 (2.032) | 20 115 | 65 (1651) | 32 (813) | 0.0274 | 0.0410 | exact, a |
| Polycarbonate | polycarbonate | 0.08 (2.032) | 17 000 | 55 (1397) | 25 (635) | 0.0274 | 0.0411 | exact, a |
| 6061 AL | aluminum | 0.03 (0.762) | 17 500 | 30 (762) | 10 (254) | 0.0145 | 0.0218 | exact, a |
| G10 | fiberglass | 0.09 (2.286) | 18 500 | 60 (1524) | 28 (711) | 0.0275 | 0.0412 | exact, a |

Row fields: `diameter_mm` 6.35, `rpm_nominal` = printed RPM, `ap_max_mm` =
printed DOC, `ae_max_mm` 6.35 (100 % engagement, from the announcement),
`machine_assumption` = the machine and the printed feed. The plunge has no LUT
field; `source_page` and `notes` carry it. Mahogany carries no
`hardness_value`.

The #202 rows are a decision, not a certainty. The chart prints one feed for
both cutters. The per-tooth chip is therefore 1.5× higher on the 2-flute ball
than on the 3-flute square. The notes of each row say this.

### 2.2 Printed but not transcribed (no LUT material family)

Bamboo (0.22", 19 000, 65, 30), ABS, Kydex, Nylon, Linoleum, PEEK,
Polypropylene, PVC, Styrene, UHMW, Sintra, Resin, AL/PVC panel, Carbon fiber,
Garolite, G10 XX, Phenolic, Foamcore, Renshape, 360 Brass, Lead, Magnesium,
Steel, Cork, Graphite, Limestone, HD Wax. The stored text holds all of them.

## 3. Carbide 3D against the Amana rows the LUT holds

Same diameter (6.35 mm), same flute count where the LUT has it, same
material. The ratio is Carbide 3D chipload / Amana `chipload_max_mm_tooth`.

| Cell | Carbide 3D row | mm/tooth | Amana row | Amana mm/tooth | Ratio |
|---|---|---|---|---|---|
| 1/4" 3F flat, hardwood | carbide3d-s3-201-hardwood-*-6350-3f | 0.0290 | amana-flat-hardwood-*-6350-3f-spektra (derived b) | 0.127 | **0.23** |
| 1/4" 3F flat, softwood | carbide3d-s3-201-softwood-* | 0.0302 | amana-flat-softwood-*-6350-3f-spektra | 0.127 | 0.24 |
| 1/4" 3F flat, plywood | carbide3d-s3-201-plywood-* | 0.0447 | amana-flat-plywood-*-6350-3f-spektra | 0.127 | 0.35 |
| 1/4" 3F flat, MDF | carbide3d-s3-201-mdf-* | 0.0398 | amana-flat-mdf-*-6350-3f-spektra (exact a) | 0.1524 | 0.26 |
| 1/4" 2F ball, hardwood | carbide3d-s3-202-hardwood-* | 0.0436 | amana-ball-hardwood-*-6350-2f-v7 | 0.127–0.1778 | 0.25 (0.34 against the min) |
| 1/4" 2F ball, softwood | carbide3d-s3-202-softwood-* | 0.0454 | amana-ball-softwood-*-6350-2f-v7 | 0.1778–0.2286 | 0.20 |
| 1/4" 2F ball, MDF | carbide3d-s3-202-mdf-* | 0.0598 | amana-ball-mdf-*-6350-2f-v7 | 0.1524–0.2032 | 0.29 |
| 1/4" flat, 6061 | carbide3d-s3-201-aluminum-* (3F) | 0.0145 | amana-zrn-flat-aluminum-pocket-6350-2f | 0.127–0.1778 | 0.08 |
| 1/4" flat, acrylic | carbide3d-s3-201-acrylic-* (3F) | 0.0290 | amana-zrn-flat-acrylic-pocket-6000-2f | 0.1016–0.1524 | 0.19 |

The operator's cell (6 mm 2-flute flat, hardwood, 18 000 rpm): the card ships
0.131 mm/tooth and 4720 mm/min. The Carbide 3D chart feeds the 1/4" cutter at
1651 mm/min in mahogany. The **feed ratio is 0.35**. The per-tooth ratio is
0.23 because the chart cutter has 3 flutes. At 2 flutes and 18 000 rpm the
chart feed gives 0.046 mm/tooth, a ratio of 0.35 against 0.131.

**The 3.175 mm cell has no Carbide 3D Shapeoko row.** The Shapeoko chart
prints 1/4" only. No ratio exists for 1/8" on this machine class.

## 4. Excluded evidence: the Nomad 883 chart

The companion chart prints #101 / #102 at .125" for the Nomad 883, a benchtop
enclosed mill with a 10 000 rpm class spindle. The header labels #101 "Square"
and #102 "Ball"; the current catalogue is the reverse (#101 ball, #102 flat).
It is a different machine class, so it is not in the LUT.

| Nomad cell (2F, 3.175 mm) | RPM | Feed ipm | mm/tooth | Amana 3.175 row | Ratio |
|---|---|---|---|---|---|
| Mahogany | 9200 | 75 | 0.1035 | amana-flat-hardwood-*-3175-2f-spektra 0.1016 | 1.02 |
| Pine | 4500 | 72 | 0.2032 | amana-flat-softwood-*-3175-2f-spektra 0.1016 | 2.0 |
| MDF | 4000 | 75 | 0.2381 | amana-flat-mdf-*-3175-2f-spektra 0.127 | 1.9 |
| Plywood | 7800 | 50 | 0.0814 | amana-flat-plywood-*-3175-2f-spektra 0.1016 | 0.80 |

The Nomad chips match or exceed Amana. The Shapeoko chips are about a quarter
of Amana. One vendor, two machines, a 4× difference: the machine, not the
cutter, sets the Shapeoko number. This is the G9 claim, measured on one
vendor's own data.

## 5. What Carbide 3D says about the machine

The chart names the **Shapeoko 3** with a trim-router spindle (Carbide Compact
Router / Makita RT0701, Dewalt DWP611 dial table). It prints nothing about the
Shapeoko 4, Pro, 5 Pro, HDM or a VFD spindle; the chart is older than all of
them. The announcement states "100% engagement" and "roughing and waterline
cutting toolpaths", and "These aren't aggressive promotional numbers, and
these aren't super conservative". No official page gives a per-machine
correction. The operator's Shapeoko Pro XXL (linear rails, belt drive) is
stiffer than a Shapeoko 3. The chart is therefore a lower bound for it, not a
match.

## 6. Scorer consequence today and the recommendation

The loader has one free-text place for the machine, `machine_assumption`. The
scorer (`feeds/vendor_lookup.rs::score_observation`) reads neither it nor
`source_vendor`. The rows carry `[machine_class=hobby_gantry_router]
[cutter=201|202]` in `notes`.

A Python copy of the scorer (tool family, row kind, grade, flutes, diameter,
pass role, material; no subfamily term, so it approximates) gives this with
the new rows embedded:

| Query (pocket, roughing) | Winner | mm/tooth |
|---|---|---|
| 6.35 mm 2F flat, hardwood | amana-compression-wood-pocket-6350-2f (unchanged) | 0.0787 |
| 6.35 mm 3F flat, hardwood | **carbide3d-s3-201-hardwood-pocket-6350-3f** (was Spektra) | 0.0290 |
| 6.35 mm 3F flat, MDF | amana-flat-mdf-pocket-6350-3f-spektra (tie; the id breaks it) | 0.1524 |
| 6.35 mm 2F ball, hardwood | amana-ball-hardwood-pocket-6350-2f-v7 (tie; the id breaks it) | 0.1778 |
| 6.0 mm 2F flat, hardwood (the operator's cell) | an Amana row (unchanged). The copy picks amana-compression-wood-pocket-6350-2f; the live card ships the Spektra 0.131, so the copy misses a term. No Carbide 3D row wins either way. | 0.0787 (copy) / 0.131 (card) |

The flute count and the observation id decide the winner, not the machine.
The Carbide 3D rows win only on 3-flute queries, and they win for every
machine profile, an industrial router too. The operator's cell does not
change. That is the argument for a typed field.

Recommendation for the lookup:

1. `MachineProfile` carries `machine_class` (enum: `hobby_gantry_router`,
   `benchtop_mill`, `industrial_router`, …) and `machine_vendor`
   (`Option<Vendor>`).
2. `VendorObservation` carries `machine_class: Option<MachineClass>` (serde
   default `None`). A `None` row is a cutter-vendor chart, written for an
   industrial router.
3. `LookupQuery` carries the profile's class and vendor. A row whose
   `source_vendor` equals the machine vendor **and** whose `machine_class`
   matches outranks a cutter-vendor row: add a term larger than the flute and
   diameter terms together (> 280). A row whose `machine_class` differs from
   the query's class is a non-match, so the Shapeoko rows never reach an
   industrial profile.
4. The card names the row: "Carbide 3D Shapeoko 3 chart (machine vendor), #201
   Mahogany". The card states the chart is withdrawn and older than the
   operator's machine.
5. Carry the chart's diameter to the query's diameter only inside the R1
   range below. Outside it, the R1 rule refuses; the lookup does not fall back
   to a cutter-vendor row silently on a hobby profile.

Until the field lands, the 44 rows change only the 3-flute 1/4" queries. The
orchestrator can take them out of `EMBEDDED_FILES` if that is not wanted.

## 7. The range (for the R1 rule)

The machine-vendor rows cover exactly this:

- Machine: Shapeoko 3 class (hobby gantry router, trim-router spindle,
  17 000–24 200 rpm printed).
- Cutters: #201, 6.35 mm, 3-flute square; #202, 6.35 mm, 2-flute ball.
- Materials: pine (softwood), mahogany (hardwood), plywood (one row), MDF,
  acrylic, HDPE, Delrin, polycarbonate, 6061 aluminium, G10.
- Engagement: 100 % (slot), roughing, one DOC per material.

Not covered: 1/8" and 1/16" cutters, V-bits (#301, #302, #501, #502), the
#251 downcut, the single-flute #274Z / #278Z, compression and tapered cutters,
finishing passes, stepover, any hardwood species except mahogany, HDF and
particleboard, and every machine after the Shapeoko 3.

## 8. Open

- `test_embedded_loads_all_observations` asserts 389 rows. With this file
  embedded the count is **433** (389 + 44). The orchestrator updates the
  assertion and reruns `lut_resolver_census_a6` and `lookup_parity`.
- The typed `machine_class` field (§6) is not implemented.
- Other hobby machine vendors named in PLAN.md G9 (Onefinity, Sienci) are not
  fetched.
