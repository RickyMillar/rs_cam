# G10 fetch, part `hobby`: notes

Part scope: PreciseBits, Carbide 3D, Sienci, Onefinity, ShopBot, the CAM
defaults of Vectric / Fusion / Carbide Create, and the literature.
Accessed 2026-09-25. The builder `scripts/g10_hobby_build.py` writes
`hobby_sources.json` (21 records, 5 of them dead ends) and
`hobby_statements.json` (143 statements). `g10_check_statements.py`
reports 0 failures for this part.

## 1. Result per target

| Target | Result | Best witness |
|---|---|---|
| Maximum ramp angle, per family / flutes / centre cutting / D | **Not published** by any hobby vendor. Only a CAM-article citation of OSG, 10-20 deg in "tough materials" (metal, grade c). | `cnccookbook_helical_ramp_angle` |
| Recommended ramp angle | **Partly.** CAM defaults only: a user reports that the Carbide Create default is 20 deg. CNCCookbook reports 1.5-2.5 deg as common (metal). No vendor document prints an angle for wood. | `carbide3d_community_ramp_entry_angle`, `cnccookbook_helical_ramp_angle` |
| Helix diameter as a fraction of D | **Partly** (CAM help, grade c). Fusion: a helix diameter bigger than D leaves a boss. The figure captions compare 1.8 x D (boss) with 0.8 x D. No minimum value printed. | `fusion_adaptive_roughing_reference` |
| Helix pitch / helix ramp angle | **Not published.** Fusion defines "Ramping Angle" and "Maximum Ramp Stepdown" per revolution, but prints no value. | - |
| Plunge feed against side feed | **Found** (grade a, wood). Sienci prints a plunge per row: plunge/feed = 0.50 for flat, ball, round-groove and most tapered rows; 0.33 for V-bits. Carbide 3D S3 (1/4 in): 0.375-0.533; Nomad (1/8 in): 0.24-0.44. | `sienci_feeds_speeds_metric`, `carbide3d_s3_feeds_250` |
| Plunge feed, absolute, wood | **Found.** PreciseBits bull nose 3F: 75 / 50 / 40 in/min by Janka. PreciseBits V-tips: 40-250 IPM (1/4 in shank), 5-25 IPM (micro). Sienci page prose: 100-300 mm/min. ShopBot: about 0.5 in/s. | `precisebits_fret_plane`, `precisebits_vtip_2500` |
| Ramp feed against side feed | **Partly** (CAM defaults). Carbide Create Pro: max(feed / 3, plunge). Vectric: the ramp runs at the plunge rate. Fusion: a separate "Ramp Feedrate" field, no default. | `carbide3d_community_ramping_cc_pro`, `vectric_v12_profile_toolpath` |
| Wood / MDF / plywood specific | **Found** for plunge only (Sienci, Carbide 3D, PreciseBits fret plane, ShopBot). Nothing for ramp or helix. | as above |
| Small tools (PreciseBits micro) | **Partly.** Micro V-tip plunge 5-25 IPM. Tapered ball: "Plunge rate = depends on speed (RPM)", no number. | `precisebits_vtip_scoreengrave` |
| Must not plunge / ramp | **Partly.** No hobby vendor forbids plunge or ramp for V-bits or tapered tools; PreciseBits and Sienci print a plunge rate for both. Only prose: a non-centre-cutting end mill cannot plunge (CNCCookbook; a Carbide forum user); Vectric's Angle option is for "cutters that cannot plunge vertically". | `cnccookbook_helical_ramp_angle`, `vectric_v12_profile_toolpath` |

## 2. Per source

- `precisebits_fret_plane`: MM3I8 3-flute corner-radius (bull-nose) cutter,
  3.0 mm / 0.125 in, r 0.64 mm. Plunge by Janka: softwood (< 1,500) 75
  in/min; 1,500-2,500: 50 in/min; > 2,500: 40 in/min. Derived: 1905 / 1270 /
  1016 mm/min (x 25.4). The feed is on a JS tab. The page prints no ratio.
- `precisebits_vtip_2500`: EM2E4 2-flute V-tip, 0.015 in tip, 1/4 in shank.
  "Plunge rate = 40 IPM to 250 IPM", one range for all materials.
- `precisebits_vtip_scoreengrave`: EM2E8 micro V, 0.005 in tip. Plunge 5-25
  IPM. "plunge style tip geometry": the vendor designs it to plunge.
- `precisebits_tapered_ball_2f`: "Plunge rate = depends on speed (RPM)". No
  number.
- `precisebits_point_styles`: the ball end is a "center cutting plunge
  point". The fish-tail "minimum 20°angle" is a tip angle, not a ramp angle.
- `carbide3d_s3_feeds_250`: Shapeoko 3, #201 3F square / #202 2F ball, 1/4
  in, one row for both tools. Wood FEED / PLUNGE (ipm): Bamboo 65/30, Pine
  75/40, Mahogany 65/32, Plywood 100/50, MDF 80/30. Derived plunge/feed:
  0.462, 0.533, 0.492, 0.500, 0.375.
- `carbide3d_nomad883_feeds_125`: Nomad 883, #101 / #102, 1/8 in. Wood FEED
  / PLUNGE (ipm): Bamboo 55/23, Pine 72/32, Mahogany 75/19, Plywood 50/22,
  MDF 75/18. Derived plunge/feed: 0.418, 0.444, 0.253, 0.440, 0.240. G9
  excluded this chart from the LUT (a different machine class).
- `carbide3d_community_ramping_cc_pro`: robgrz (Carbide 3D staff; the
  Discourse JSON staff flag is true), 2022-09-29: "If ramping is enabled,
  the feedrate is 1/3 of the feerate, or 100% of the plunge rate, whichever
  is higher." CC Pro pockets use a helix when the helix fits.
- `carbide3d_community_ramp_entry_angle`: a user says the Carbide Create
  default is 20 deg. A forum regular (not staff) says a solid-centre tool
  must not ramp or plunge.
- `sienci_feeds_speeds_metric`: Sienci chart, 4 pages. This part
  transcribes the two wood pages: 106 rows, each with feed, plunge and a
  derived plunge/feed fraction. Summary of the derived fractions:
  - flat (UC, DC, single flute, corncob, surfacing), ball, round groove:
    0.498-0.505 on both wood pages;
  - tapered ball: 0.498-0.504, except "1/4" - D1/16" ... Fine" on the soft
    page, which is 0.334 (1270 / 3800); the hard page prints 1900 / 3800 =
    0.500 for the same tool;
  - V-bit: 0.330-0.335 (750 / 2240, 580 / 1730, 370 / 1120).
  The absolute plunge on the flat rows is 430-1370 mm/min.
- `sienci_lm_feeds_and_speeds`: "lower plunge rates (100mm/min to
  300mm/min) for most materials", for 2F 1/8 in end mills. This value is
  2-5 x lower than the same vendor's chart. The two documents disagree.
- `vectric_v12_profile_toolpath`: "All ramp moves are performed at the
  plunge rate selected for the current tool." Ramp by distance or angle; no
  default angle printed. The Spiral option computes its angle from the
  pass depth and the perimeter.
- `vectric_tool_database_v11`: the Plunge Rate is also the ramp rate. The
  raw sha256 (dc4881ed...) equals the G5 copy of 2026-09-24.
- `fusion_adaptive_roughing_reference`: parameter definitions with no
  defaults. The helix diameter must not exceed D, or a boss stays. The
  figure captions show 1.8 x Dia (bad) and 0.8 x Dia. Derived: 0.8 x D
  diameter = 0.4 x D radius. rs_cam today uses 0.3 x D radius (0.6 x D
  diameter). Fusion keeps separate Ramp Feedrate and Plunge Feedrate
  fields.
- `cnccookbook_helical_ramp_angle`: plunge = slot feed / flutes (2F gives
  0.5). Common ramp angles 1.5-2.5 deg. OSG (second hand): 10-20 deg.
  Plunge needs a centre-cutting end mill. A 4F non-centre-cutting end mill
  has a ramp-angle limit. Metal context.
- `shopbot_user_guide_2015`: for most woods, XY 1.7 in/s start and Z plunge
  about 0.5 in/s. Derived: 762 mm/min; 0.5 / 1.7 = 0.294.

## 3. Dead ends

| What | URL | Status / reason |
|---|---|---|
| Carbide 3D S3 chart, Wayback | https://web.archive.org/web/20211118030340id_/https://docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf | curl timeout (000) twice on 2026-09-25. The G9 bytes are used; the sha256 matches. |
| Carbide 3D Nomad chart, live | https://docs.carbide3d.com/support/supportfiles/Nomad883_feeds_125.pdf | 200, but the body is an HTML page (38 134 B). The G9 bytes are used. |
| Carbide Create v5 manual | https://guides.carbide3d.com/files/pdf/carbide-create-v5.pdf | 200, 36 pages, no "plunge" / "ramp" / "helix". |
| Carbide 3D guides, toolpaths | https://guides.carbide3d.com/create/toolpaths/ | 200, 7.6 kB, no entry parameters. |
| Carbide Create tool library | local ccpro.db (G9 §0) | encrypted; not read. |
| ShopBot Feeds and Speeds Charts 2016 | https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf | 200, 12 pages; feed / RPM / chip load only. |
| Sienci lmk2 page | https://raw.githubusercontent.com/Sienci-Labs/Resources/main/lmmk2/lmk2-handbook/lmk2-feeds-and-speeds.md | 404 (path moved). The LongMill page and the chart PDF replace it. |
| Sienci LongMill page, first path | https://raw.githubusercontent.com/Sienci-Labs/Resources/main/longmill/the-basics/lm-feeds-and-speeds.md | 404; the correct path is `longmill/lm-the-basics/`. |
| Sienci handbook | https://raw.githubusercontent.com/Sienci-Labs/Resources/main/cnc-fun/handbook/cnc-feeds-speeds.md | 200; only "avoid taking straight plunges" for acrylic and a tapered-bit breakage warning. Not stored (not wood entry data). |
| PreciseBits chip-breaker router page | https://www.precisebits.com/products/carbidebits/fcrouter.asp | 200; no plunge / ramp text. |
| PreciseBits site search | WebSearch `site:precisebits.com ramp` | only "feedrate ramp (or acceleration)"; no ramp entry guidance. |
| Onefinity | https://forum.onefinitycnc.com/t/feeds-and-speeds-guide-for-beginners/3780 | forum threads and a user calculator only; no vendor document. |
| Fusion ramp angle default (2 deg) | https://forums.autodesk.com/t5/fusion-manufacture-forum/ramp-angle-amp-plunging-approaches/td-p/12816036 | forum only; the Fusion help prints no default. Not stored. |
| Sandvik Coromant ramping | https://www.sandvik.coromant.com/en-gb/knowledge/milling/milling-holes-cavities-pockets/ramping | downloaded, then removed: the `metal` part holds it (`metal_sandvik_ramping`). |
| Machinery's Handbook, Koch "Wood Machining Processes" | - | not available online. |
| Wood entry papers | Crossref, Semantic Scholar (queries in §4) | Semantic Scholar API returned no data (rate limit); Crossref returned no study of ramp, helix or plunge entry in wood. |

## 4. Searches

WebSearch:
- `precisebits ramp angle helical interpolation plunge`
- `site:precisebits.com ramp`; `site:precisebits.com "plunge rate"`
- `Carbide Create ramp angle default plunge rate documentation`
- `docs.carbide3d.com plunge rate feeds speeds "plunge"`
- `"Carbide Create" manual "ramp" angle toolpath option carbide3d.com`
- `Fusion 360 ramp taper angle helical ramp diameter default documentation`
- `Vectric Aspire ramp plunge moves ramp angle distance spiral documentation`
- `Sandvik Coromant ramping maximum ramp angle solid carbide end mill helical interpolation diameter`
- `Sienci Labs resources feeds and speeds plunge rate ramp LongMill`
- `ShopBot ramp plunge "ramp" feed speed wood recommended documentation shopbottools`
- `Onefinity CNC feeds speeds plunge rate chart wood`
- `helical milling entry ramp angle wood CNC router study paper plunge MDF tool`
- `Machinery's Handbook ramping angle end mill "ramp angle" helical interpolation guidance pdf`

Crossref (`api.crossref.org/works?query=`): `helical milling wood`,
`helical milling medium density fiberboard`, `plunge milling wood router`,
`ramping entry milling tool life`. The nearest hits are wood edge-milling
and helix-angle chip studies (for example 10.1007/bf00767289, 2002; and
10.1007/s00107-010-0517-8, 2011). They are about the tool helix angle in
side milling, not the entry move. Semantic Scholar: the same four queries
returned no data.

## 5. For the verifier

1. The Sienci rows get their tier (Reduced / Regular / Full) from the
   order of the "End Mills" headers on each page. The tier labels in the
   pdftotext layout sit in the middle of each block. Check two rows
   against the PDF: "1/4" Flat End Mill UC 1650 830" is Reduced, and
   "1/4" Flat End Mill UC 2190 1100" is Regular.
2. The Sienci verbatims ("<name> <feed> <plunge> <stepover>") occur on both
   wood pages when the rows are the same. `source_page` gives the page.
   The page-2 values match page 1 for most end-mill rows. The stepdowns
   differ, but the stepdowns are not transcribed here.
3. The Carbide 3D rows are shared by a square and a ball cutter; the
   header that names the tools is an image (G9). All rows carry
   `tool_family` `flat_end_mill`, grade b. The ball reading is the same
   row.
4. Carbide 3D S3 "Pine .4″" DOC is printed as .4 in on a 1/4 in tool. It is
   transcribed as printed, and this part does not use it.
5. The Carbide Create statements are CAM defaults (grade c). The 20 deg
   default comes from a user post, not from Carbide 3D. The ramp-feed rule
   comes from a staff post.
6. The Fusion "0.8 x the Dia" verbatim joins two figure captions across
   table cells ("Value of 1.8 x the Dia | | Value of 0.8 x the Dia").
   Fusion prints these values as examples, not as a default.
7. The PreciseBits pages change their page chrome between fetches. The
   fret-plane raw sha256 on 2026-09-25 (7ea1ad3f...) differs from the G6
   record (6c6797f7...), but the plunge lines are the same.
8. `g5_html_to_text.py` writes `accessed_on: 2026-09-24`. This part changed
   that header line to 2026-09-25 in its own text files before it hashed
   them.
9. Derived fractions are rounded to 3 decimals. The formula in each
   `derived` entry gives the printed operands.
