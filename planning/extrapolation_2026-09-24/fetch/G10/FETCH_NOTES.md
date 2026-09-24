# G10 fetch notes (merged)

Date: 2026-09-25. Group: G10, entry parameters. This file merges the three
part notes and the two verifier reports. The part files hold the detail:

- `parts/wood_NOTES.md` (Onsrud, Amana, Whiteside, Vortex, IDC, ToolsToday,
  Freud, AXYZ; 47 statements)
- `parts/metal_NOTES.md` (Harvey, Helical, Garr, Kennametal, SGS, IMCO,
  Sandvik; 53 statements)
- `parts/hobby_NOTES.md` (PreciseBits, Carbide 3D, Sienci, ShopBot, Vectric,
  Fusion, CNCCookbook, literature; 143 statements)
- `verify/a_REPORT.md` (wood and metal), `verify/b_REPORT.md` (hobby)

The merged data: `statements.json` (243) and `sources.json` (60 records, 43
with a stored text). `scripts/g10_verified.py` writes them from the parts and
the verdicts. `scripts/trend_g10.py` prints the Phase 2 tables
(`trend_g10.out`).

## 1. Result per target

"Grade c" means metal, plastics, CAM help or prose. It is context for wood,
not a source.

| Target | Wood / router vendor | Hobby (wood) | Metal (grade c) |
|---|---|---|---|
| Maximum ramp angle | **not published** | not published (a user post: Carbide Create default 20 deg) | Harvey/Helical 3-10 deg soft, 1-3 deg hard; SGS per series 1-90 deg; SGS compression router 5, up-cut router 90, down-cut router "–" |
| Ramp angle vs flutes, centre cutting | not published | not published | SGS: centre-cutting 2-3 flute 90 deg; non-centre-cutting and 4-7 flute 1-7 deg. No formula |
| Helix diameter / radius | not published | Fusion: helix diameter <= D (path frame); example 0.8 x D | Harvey/Helical ">110-120%" (frame not defined); IMCO bore <= 2D - 2rc; Sandvik hole <= 2 x D3 |
| Helix pitch / helix angle | not published | not published (Fusion fields without values) | IMCO helix angle per series 0.5-5 deg; Sandvik pitch <= max ap |
| Plunge vs side feed | IDC plunge column 0.05-0.57; Amana "Ramp Down" = F / Z | Sienci 0.50 (flat, ball, tapered ball), 0.33 (V-bit); Carbide 3D 0.24-0.53 | Garr 0.5; SGS 0.25; Harvey chamfer 0.40-0.50, engraver 0.50 |
| Plunge, absolute | - | PreciseBits bull 75/50/40 in/min by Janka; V-tips 40-250 and 5-25 IPM; ShopBot about 0.5 in/s | - |
| Ramp feed vs side feed | Amana Ramp Down = F / Z (meaning not defined) | Carbide Create Pro max(F/3, plunge); Vectric ramp at plunge; Fusion separate field | SGS 1.0 at 1-2 deg, 0.25 near 6 deg; Sandvik linear 0.75; IMCO 1.0 (M series) |
| Must not plunge / should ramp | Vortex down-cut "CANNOT ... plunge straight into wood"; AXYZ compression "best to ramp"; Onsrud PCD "TIPICALLY CANNOT PLUNGE" | no hobby vendor forbids a plunge for any tool | non-centre-cutting tools must not plunge (Harvey, Helical, CNCCookbook); SGS Z5 "Do not plunge."; SGS "plunging not recommended" in M, S, H classes |
| V-bit / tapered tool entry | Amana and IDC print a ramp-down or plunge feed for V-bits | Sienci and PreciseBits print a plunge for V-bits and tapered balls | Harvey engraver: plunge at 50 %, "ramping is preferred" |
| Helix start clearance | **not published** | **not published** | **not published** |
| Wood entry literature | - | Crossref: no study of ramp, helix or plunge entry in wood; Semantic Scholar rate-limited | - |

## 2. Per part, in short

- **wood.** Amana prints the only wood-vendor entry numbers: "To find Ramp
  Down: Feed Rate IPM / # of flutes" on six charts, a column on three
  (compression v8, Spektra v24, V-groove v16). IDC prints a plunge column for
  every family. Onsrud, Whiteside and Vortex print prose only.
- **metal.** Every value is grade c for wood. No document in the part names
  wood in an entry rule. The Harvey wood charts (SF_809500, SF_809100) print
  no entry value.
- **hobby.** Sienci gives 106 wood rows with a plunge and a feed. Carbide 3D
  S3 and Nomad give 10 wood rows (one row serves a square and a ball tool).
  The CAM rules (Vectric, Carbide Create, Fusion) are conventions, grade c.

## 3. Verification

| Part | Statements | confirmed | wrong | grade_wrong | Sources rehashed |
|---|---|---|---|---|---|
| wood + metal (a) | 100 | 96 | 3 | 1 | 21 match, 8 changed (HTML chrome only; body text identical), 10 dead ends |
| hobby (b) | 143 | 142 | 0 | 1 | 11 match, 2 changed (Discourse HTML; quotes verbatim), 3 unreachable (checked against the stored PDF) |

The corrections are in `statements.json` (`verdict`, `corrected_from`):

- `g10-metal-harvey-sf18700-plunge` (wrong): diameter range is 0.381-12.7 mm.
- `g10-metal-harvey-sf25000-plunge`, `-ramp-pref` (wrong): the material
  includes plastics, not metals only.
- `g10-wood-vortex-veining-plunge` (grade_wrong): a veining bit with a top
  roundover, not a ball nose.
- `g10-hobby-019` (grade_wrong): Fusion 0.8 x D is an example, not a maximum.

Verifier findings that stay open (not fixed in the data):

1. Amana V-groove v16: every 1-flute row from 40 to 110 deg prints Ramp Down
   = Feed / 2, not Feed / 1 as the footer rule says. RC-1146 and RC-1101 print
   Ramp = Feed and do not agree with their own chip load (verifier a 4.1).
2. No Amana document defines "Ramp Down" (verifier a searched 33 text files).
3. SGS down-cut router "–" has no legend. SGS router rows are colour-coded
   "Non Ferrous N5 to N7" only (verifier a 4.4).
4. `g10-metal-sgs-mx-upcut`, `-downcut`: the matrix prints 2 flutes; the
   statements have `flutes` null.
5. Sienci: the live web table differs from the PDF on 5 tapered-ball rows
   (web plunge lower, about 0.33 on the soft page). The record uses the PDF
   (verifier b 4.1).
6. Sienci page prose "100mm/min to 300mm/min" disagrees with its own chart
   (about 0.5 x feed, 430-1370 mm/min). The chart is newer.
7. Carbide 3D Nomad: the text header says the sha256 equals "the G9 record",
   but `fetch/G9/sources.json` has no Nomad record. Only the G9 file matches.
8. Fusion help shows "2°" in its ramping-angle figure, as an example only.
   The hobby dead-end table says the help prints no ramp angle.
9. Five metal text files carry `accessed_on: 2026-09-24` from the shared
   `g5_html_to_text.py`. The real access date is 2026-09-25.
10. Two grade c claims found no primary: "a ramp-in entry angle of no more
    than 10°" (Burnette reseller, cites no Amana document) and "45 degree ...
    1/3 of the calculated feed rate" (Cle Bit Co, not Vortex). Neither is
    stored as a vendor source.

## 4. Dead ends

| Vendor | Result |
|---|---|
| Amana product and technical pages | HTTP 403 |
| Onsrud technical pages | 404; the stored articles and the PCT-19 catalogue print no wood entry numbers |
| Whiteside | no technical pages (404); the catalogue has "plunge" in product names only |
| Vortex | technical pages 404; the chip-load chart is image-only, chip loads only |
| Freud CNC chart 2017 | chip loads only |
| Kennametal | 6 URLs (403, 404, product pages "Ramping: Blank", a calculator); prose blog only |
| Helical S&F index, Lakeshore | no entry text |
| Carbide Create v5 manual, guides, tool library | no entry values; the library is encrypted |
| ShopBot charts 2016 | feed, RPM and chip load only |
| Onefinity | forum threads only |
| Machinery's Handbook, Koch | not available online |

## 5. Searches

The part notes list every query: wood 13 (`parts/wood_NOTES.md` §4), metal 15
(`parts/metal_NOTES.md` §4), hobby 13 web queries plus 4 Crossref and 4
Semantic Scholar queries (`parts/hobby_NOTES.md` §4).
