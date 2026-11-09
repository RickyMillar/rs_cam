# Large straight-flute router bits: published limits and loads (2026-11-10)

Research report for the feeds contract. Read-only on the code. Access date for
every web source: 2026-11-10. Grades: `a` = standard or peer-reviewed, `b` =
vendor table or vendor product page, `c` = vendor prose, retailer page, blog or
forum. A row marked **snippet** was read in a web-search result only; the page
itself refused the fetch (HTTP 403, 404, 504 or a bot wall). Treat a snippet row
as a lead, not as a stored source, until a later session fetches the page.

Tool in question: 18 mm diameter, 2 flutes, 20 mm flute length, 6 mm shank,
straight cutting edges, white oak, 24000 rpm spindle.

Fetch budget: 25 web fetches used (the brief set about 25). Two PDFs came back
as bytes and were read locally with `pdftotext -layout`: Freud 2017 and Curti
2021. One further PDF (Porankiewicz 2021) was read the same way.

## 0. Sources already in the repo that this report builds on

| Key | Where | Use here |
|---|---|---|
| `freud_router_cnc` | `tests/literature_matrix/sources.toml`, CREDITS.md | The carbide-tipped straight-bit chip table (section 2) is in this sheet. No text copy was stored before; this report prints it. |
| `onsrud_hwood` | `data/vendor_lut/sources/onsrud_hard_wood_cutting_data.txt` | The 40-100 and 48-000 rows are straight carbide-tipped series (section 2). The `*`/`**` footnotes print two RPM maxima (section 1). |
| `whiteside_wood`, `whiteside_chipload` | `sources.toml` | Whiteside prints no chip card. This report adds the product-page RPM lines. |
| Curti 2021 force line | `material/force_line.rs`, `material/mod.rs` | The paper measures a straight blade (helix 0) as one of three tools (section 3). |

## 1. Maximum RPM versus cutting diameter

### 1.1 What was found

| # | Source | Grade | Diameter and RPM printed | Quoted line |
|---|---|---|---|---|
| 1a | Freud, *Get the most out of your Freud bits by routing at ideal feed rates and speeds* (2017-08-22), https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf , fetched, PDF read with pdftotext | b | No RPM table by diameter. | "To reduce the risk of injury, always check to ensure the rated speed of the router bit is higher than or equal to the maximum speed marked on the CNC machine. Router bits running faster than their RATED SPEED can break and fly apart." |
| 1b | Freud 04-140, 3/4 in diameter x 3/4 in cut, 1/4 in shank, double-flute straight bit. Retailer listings found by search (contractorcave.ca, 1toptools.com, toolnut.com); freudtools.com product page not found by search | c, snippet | 3/4 in (19.05 mm): 24,000 rpm max | "Max RPM: 24,000 \| Overall length: 2-1/8″ \| 1/4″ shank" |
| 1c | Whiteside Machine Co., product 1072, https://www.whitesiderouterbits.com/products/1072 , fetched HTTP 200 | b | 1/2 in CD, 1/2 in SH: 16,000-18,000, max 24,000 | "1/2\"SH, 1/2\"CD, 2\"CL" ... "Recommended RPM: 16,000-18,000 (max 24,000)" ... "Two flute, carbide-tipped, straight bit" ... "Designed for hand-held routers, router tables, and CNC machines" |
| 1d | Whiteside product 1071, https://www.whitesiderouterbits.com/products/1071 , fetched | b | 1/2 in CD: same line as 1c | "Recommended RPM: 16,000-18,000 (max 24,000)" |
| 1e | Whiteside product 3002 (template bit), https://www.whitesiderouterbits.com/products/3002 , fetched | b | 1/2 in CD, 1/4 in SH: 16,000-18,000, max 24,000 | "1/4\"SH, 1/2\"CD, 3/4\"CL" ... "Recommended RPM: 16,000-18,000 (max 24,000)" ... "MACHINES: Designed for use in hand-held routers and router tables" |
| 1f | Whiteside 3/4 in cutting-diameter product pages (roughing down-cut spiral 3/4 in SH 3/4 in CD; "CNC Router Bit 3/4\"SH, 3/4\"CD, 1-1/2\"CL"; template bit 3/8 in SH 7/8 in CD), whitesiderouterbits.com, found by search | b, snippet | 3/4 in CD: 12,000-14,000 recommended, 18,000 max. 7/8 in CD: 12,000-14,000, 18,000 max | search snippet: "12,000–14,000 RPM, with a 18,000 max" |
| 1g | LMT Onsrud, *Hard Wood Cutting Data Recommendations*, stored at `data/vendor_lut/sources/onsrud_hard_wood_cutting_data.txt` (repo) | b | Series 37-80 (carbide-tipped lettering bits): 1 1/4 in column "`.004-.006*`", 2 in column "`.004-.006**`" | "* = 16,000 RPM" and "** = 15,000 RPM" (the only RPM maxima on the sheet; they are per series, not a diameter table) |
| 1h | Amana Tool, *Max RPM Guide for Amana Tool Router Bits*, https://www.amanatool.com/maxrpm , HTTP 403 to fetch | b, snippet | "2\" (less than 50mm)": 28,000; larger sizes lower (rows not read) | "refer to laser etching on router bit shank for most updated RPM speed rating" and "you should not exceed the maximum RPM shown for the particular tool you are using" |
| 1i | Amana 45251, carbide-tipped straight plunge 3/4 in D x 1 in CL x 1/4 in shank, 2 flutes. Retailer burnstools.com, https://www.burnstools.com/45251-3-4d-x-1cl-x-1-4shnk-straight-2fl , found by search | c, snippet | 3/4 in on 1/4 in shank: 35,000 rpm max | search snippet: "maximum speed of 35,000 RPM" |
| 1j | Wealden Tool (UK retailer), *Cutter speeds*, https://www.wealdentool.com/kb/quick-tips/routing-basics/cutter-speeds/ , fetched HTTP 200 | c | Up to 25 mm: 24,000. 25-50: 18,000. 50-65: 16,000. 65-75: 12,000. Over 75: 10,000 | "Up to 25mm 24,000rpm", "Over 25mm-50mm 18,000rpm", "Over 50mm-65mm 16,000rpm", "Over 65mm-75mm 12,000rpm", "Over 75mm 10,000rpm" |
| 1k | Wealden Tool, *Router cutters - use*, https://www.wealdentool.com/kb/rons-tips/cutter-care-and-use/router-cutters-use/ , fetched | c | Same bands as 1j | "Diameter up to 25mm – maximum speed 24,000rpm" ... "Many cutters are designed for use in a router table at a specific speed. These speeds should be adhered to and such cutters never used in a hand-held router." |
| 1l | Trend Tool Technology, *Router Cutter Terminology*, https://www.trend-usa.com/router-cutters-terminology , fetched; the table sat past the 40k-character read limit, so the rows come from the fetch tool's own summary, not verbatim | c, summary | Up to 24 mm: 28,000. 25-30: 24,000. 31-50: 18,000. 51-67: 16,000. 68-86: 12,000 | summary text: "Cutters over 40mm diameter should normally only be used in a router that is either fixed under a table or overhead in a machining centre." |
| 1m | CMT Orange Tools router-bit leaflet, https://www.scosarg.com/media/leaflets/cmt/CMT%20Router%20Bits.pdf , HTTP 403 | b, snippet | Flush-trim range: 19 mm 18,000; 35 mm 16,000 | search snippet: "19mm bits Max RPM 18.000, 35mm bits Max RPM 16.000" |
| 1n | EN 847-1:2005, *Tools for woodworking - Safety requirements - Part 1: Milling tools, circular saw blades*, iTeh catalogue page https://standards.iteh.ai/catalog/standards/cen/07bbe356-dde3-44da-9220-d5912d780eb3/en-847-1-2005 , not fetched | a, snippet | No table. The standard gives a formula for the maximum rotational speed and requires the maker to mark it. | search snippet: "This document does not apply to boring bits, eccentric single router cutters, cutters with cutting circle less than 16 mm" |
| 1o | Amana Tool news, *Router Bits Manufactured According to European Safety Standard EN-847-1/2*, https://www.amanatool.com/news/index/view/id/21 , not fetched | c, snippet | None | search snippet: each tool has "a maximum safe speed calculated from profile size, steel type, shank size, balance and machine run-out" |

Bosch and Festool: the Bosch 1617 manual copy (i3detroit.org) sits behind a bot
wall; the boschtools.com product page prints the spindle range only
("8000-25,000 RPM"). No Festool table was found in two searches. HSE and DGUV:
no primary sheet on router-cutter speed was found in two searches (queries:
`HSE woodworking information sheet "EN 847-1" tool marked maximum speed router
cutters`; `DGUV Information Fräswerkzeuge "EN 847-1" Höchstdrehzahl
Kennzeichnung`). Leitz Lexicon chapter 05 (Routing) is over the 10 MB fetch
limit; not read.

### 1.2 Is there a formula rather than a table?

- EN 847-1 (1n) is the only source that is said to hold a formula for the
  maximum rotational speed. The text was not read. The limit it gives is per
  tool, set by the maker, and printed on the shank.
- No vendor page read gives a rim-speed limit in m/s. Trend (1l) prints the
  peripheral-speed identity only, with no limit. A hobby calculator
  (woodcalcs.uk, grade c, search snippet) uses "40-60 m/s" for solid wood.
- The repo's 14000 tier and the vendor tables, as rim speed. Arithmetic: v =
  pi x D x n / 60.

| Case | Rim speed |
|---|---|
| 18 mm at 14000 rpm (repo tier `MILLING_CEILING_LARGE_RPM`) | 13.2 m/s |
| 18 mm at 24000 rpm (spindle max) | 22.6 m/s |
| 19.05 mm at 18000 rpm (Whiteside 3/4 in max, 1f) | 18.0 m/s |
| 19.05 mm at 24000 rpm (Freud 04-140, 1b) | 23.9 m/s |
| 19.05 mm at 35000 rpm (Amana 45251 retailer, 1i) | 34.9 m/s |
| 25 mm at 24000 rpm (Wealden band edge, 1j) | 31.4 m/s |
| 24 mm at 28000 rpm (Trend band edge, 1l) | 35.2 m/s |
| 50 mm at 18000 rpm (Wealden band edge) | 47.1 m/s |

The vendor bands are not a constant rim speed. The band edges run from 31 to 47
m/s. No table row reaches 14000 rpm at or below 25 mm.

### 1.3 The 18 mm (3/4 in) value, by publisher

| Publisher | 3/4 in or 18-20 mm value | Kind | Grade |
|---|---|---|---|
| Freud (04-140 straight, 1/4 in shank) | 24,000 max | retailer listing of the bit | c, snippet |
| Whiteside (3/4 in CD bits) | 12,000-14,000 recommended, 18,000 max | product page | b, snippet |
| Amana (45251 straight plunge, 1/4 in shank) | 35,000 max | retailer listing | c, snippet |
| Amana (general guide) | 28,000 for "2\" (less than 50mm)" | vendor table | b, snippet |
| CMT (flush trim 19 mm) | 18,000 max | leaflet | b, snippet |
| Wealden (UK) | 24,000 for up to 25 mm | retailer table | c |
| Trend | 28,000 for up to 24 mm | vendor page (summary) | c |
| Onsrud | none at 3/4 in; 16,000 at 1 1/4 in and 15,000 at 2 in for series 37-80 | vendor chart footnote | b |

## 2. Chip load for straight-flute bits in hardwood

### 2.1 Vendors that print a chip load for straight bits

**Freud (1a, fetched, grade b).** The sheet has two tables. Title lines, verbatim:
"CHIP LOADS FOR FREUD SOLID CARBIDE ROUTER BITS ONLY: Recommended* Chip Loads,
based on cut depth equal to bit diameter:" and "RECOMMENDED* CHIP LOADS FOR
FREUD CARBIDE TIPPED STRAIGHT AND PROFILE BITS ONLY:". Both carry the note
"*This chart is a recommended starting point and does not warranty against tool
breakage." and "Carbide tipped bits should not be used to drill directly into
the work piece."

Carbide-tipped straight and profile bits, inches per tooth, as printed (pdftotext
drops some quote marks; the layout is the sheet's):

| Tool diameter | MDF/Particle board | Laminated particle board | Hardwood | Softwood | Soft plastic | Acrylics/Hard plastic | Solid surface/Plywood | Aluminum |
|---|---|---|---|---|---|---|---|---|
| 1/8" | .002"-.004" | 003"-006" | .002"-.004" | .003"-.005" | 003"-006" | .002"-.004" | 003"-005" | N/A |
| 1/4" | .004"-.006" | .006"-.008" | .005"-.007" | .006"-.008" | .006"-.008" | .004"-.006" | .005"-.006" | N/A |
| 3/8" | .005"-.007" | .007"-.009" | .006-.008" | .008"-.010" | .007"-.009" | .005"-.007" | .006"-.008" | N/A |
| 1/2" | .006"-.007" | 008"-.010" | .008"-.010" | .008"-.012" | 008"-.010" | .006"-.007" | .007"-.009" | N/A |
| 5/8" | .006"-.007" | .009"-.011" | .008"-.012" | .009"-.013" | .009"-.011" | .006"-.007" | .008"-.010" | N/A |
| 3/4" | .007"-.009" | .010"-.012" | .009"-.013" | .010"-.012" | .010"-.012" | .007"-.009" | .009"-.011" | N/A |

Hardwood at 3/4 in: .009-.013 in = 0.229-0.330 mm per tooth. The sheet names no
RPM for the table; its worked examples use 18,000 rpm.

Solid carbide table (same sheet), hardwood column: 1/8" .002"-.005", 1/4"
.008"-.011", 3/8" .014"-.016", 1/2" .018"-.021". The solid-carbide table stops
at 1/2 in.

Freud does not say in prose that a straight bit takes a smaller chip. The two
tables say it by number. At 1/2 in, hardwood: carbide-tipped straight .008-.010
in against solid carbide .018-.021 in. Ratio of the band midpoints: 0.009 /
0.0195 = 0.46. At 1/4 in: .005-.007 against .008-.011, ratio 0.63. These ratios
are this report's arithmetic on the printed cells.

**LMT Onsrud (1g, repo text, grade b).** The hard-wood chart prints rows for
series 40-000, 40-100 and 48-000 at "1xD". Retail pages identify 48-000 as
"48-000 Carbide Tipped Straight Flute Router Bits" (wsimachinery.com, search
result) and 48-088 as "3/4 carbide tipped two flute straight v flute"
(ballewsaw.com, search result); the stored PCT-19 excerpt does not hold the
48-000 series heading, so the identification is grade c. Column mapping below
is by character position in the stored layout text; a one-column slip is
possible on the right-hand side, so re-read the PDF before any LUT entry.

| Series | 1/4 | 3/8 | 1/2 | 5/8 | 3/4 | 7/8 | 1 |
|---|---|---|---|---|---|---|---|
| 40-100 | .005-.007 | .006-.008 | .007-.009 | | .009-.011 | | |
| 48-000 | .005-.007 | .005-.007 | .006-.008 | .007-.009 | .008-.010 | .009-.011 | .010-.012 |

48-000 at 3/4 in: .008-.010 in = 0.203-0.254 mm per tooth. The chart's depth
rule applies: "1 x D Use recommended chip load / 2 x D Reduce chip load by 25%
/ 3 x D Reduce chip load by 50%".

**Not found.** Amana: no straight-bit chip chart found (the Spektra chart is
spiral). Whiteside: no chip card (repo row `whiteside_chipload`). CMT, Bosch,
Festool, Trend: no chip table found. No vendor read states in words whether a
straight bit takes a smaller or a larger chip than a spiral bit.

## 3. Straight versus spiral (helical) edges in wood

### 3.1 Curti et al. 2021 covers helix 0 as a measured tool

Source: R. Curti, B. Marcon, L. Denaud, M. Togni, R. Furferi, G. Goli,
*Generalized cutting force model for peripheral milling of wood, based on the
effect of density, uncut chip cross section, grain orientation and tool helix
angle*, European Journal of Wood and Wood Products 79:667-678 (2021),
https://doi.org/10.1007/s00107-021-01667-5 , open access copy fetched from
https://d-nb.info/1231639377/34 and read with pdftotext. Grade a.

Tools (Table 2): "Straight blade (0°)", "Helical cutter (15°)", "Helical cutter
(30°)"; each "20" mm diameter, "2" flutes, rake "25", wedge "55", clearance
"10", "Tungsten Carbide". Conditions (Table 3): "Spindle speed 3000 rpm", "Feed
rate 2000 mm/min", "Axial depth of cut 30 mm", "Radial depth of cut 0.5, 1.0,
1.5, 2.0, 2.5 mm"; "average uncut chip thicknesses h roughly varying from 40 µm
... to 100 µm". Species (Table 1): paulownia 287.1, lime 585.7, maple 623.9,
"Oak (Quercus robur)" 737.8, azobe 1079.5 kg/m3. The model holds "between 287
and 1080 kg/m3 to avoid extrapolation".

Quoted findings on helix angle:

- "The back forces are the highest for the maximum helix angle (λ = 30°)
  whereas cutting forces are maximized when machining with the straight blade
  (λ = 0°)."
- "Ks decreases with the helix angle of the tool. The differences are the lowest
  when cutting parallel to the grain and the largest when machining with the
  grain. Ks is also steadier when using a helical cutter than when milling with
  a straight blade. This observation can be due to the high dynamic excitation
  when machining with the straight blade tool."
- "It appears that cutting and resultant forces were lower with helical blades
  but back forces were higher. The cutting force was found to be directly
  proportional to the uncut chip thickness using helical blade whereas with the
  straight blade a non-negligible intercept was found."
- On why makers use a helix: "this parameter is commonly used to reduce the
  noise of cutting operations and is supposed to reduce the cutting forces by
  increasing slicing of the chip and increasing the apparent sharpness of the
  tool edge".
- Cutting continuity: the straight blade "presents a cutting continuity
  coefficient (computed as the ratio between the angle during which one blade
  is actually machining ...)" that is low; "it varies from 0.35 to 0.5 for λ =
  15° and from 0.6 to 0.8 for λ = 30°".
- Up- versus down-milling: "forces while up-milling are 9% lower than during
  down-milling with a standard deviation of about 4%".

Table 4, "Fp/Fc ratio averaged for the five depths of cut (SD in parentheses)",
oak column: λ = 0°: "0.0% (2.2%)"; λ = 15°: "18.8% (2.5%)"; λ = 30°: "39.7%
(4.5%)". Paulownia: 2.3%, 9.4%, 25.3%. Lime: 0.4%, 17.2%, 39.5%. Maple: 4.6%,
13.2%, 32.6%. Azobe: 1.8%, 20.6%, 44.8%. (Fp = back force, axial; Fc = cutting
force.)

Table 5 prints the fitted density-normalised Ks per helix angle, "GA must be
expressed in degrees". Up-milling: λ=0 `Kmodel = -5e-6 GA² + 1e-3 GA + 26e-3`;
λ=15 `-4e-6 GA² + 7e-4 GA + 32e-3`; λ=30 `-3e-6 GA² + 5e-4 GA + 21e-3`.
Down-milling: λ=0 `-3e-6 GA² + 5e-4 GA + 56e-3`; λ=15 `-3e-6 GA² + 5e-4 GA +
49e-3`; λ=30 `-2e-6 GA² + 4e-4 GA + 30e-3`. Units N/mm² per kg/m³.

Arithmetic from Table 5 at density 762 kg/m3 (the repo's white oak), this
report's math:

| Grain angle, mode | Ks at λ=0 | Ks at λ=15 | Ks at λ=30 | 0°/15° | 0°/30° |
|---|---|---|---|---|---|
| 90°, down-milling | 58.4 N/mm² | 53.1 | 37.9 | 1.10 | 1.54 |
| 90°, up-milling | 57.5 | 47.7 | 31.8 | 1.21 | 1.81 |
| 45°, down-milling | 55.2 | 49.9 | 33.5 | 1.11 | 1.65 |
| 0°, down-milling | 42.7 | 37.3 | 22.9 | 1.14 | 1.87 |

The 58.4 at GA 90°, down-milling, λ=0 matches the repo's 58.5 N/mm² at 762
kg/m³ to rounding. So the force line in use is the paper's straight-blade fit,
not a helix-0 extrapolation of a spiral fit. The paper prints no single
"straight over spiral" ratio in words; the ratios above are from its Table 5.

### 3.2 Other sources on straight versus helical edges

| Source | Grade | What it says |
|---|---|---|
| *Influence of helix angle and density variation on the cutting force in wood-based products machining*, Journal of Materials Processing Technology (2007), https://www.sciencedirect.com/science/article/abs/pii/S0924013607000726 , HTTP 403; the DOI tried (10.1016/j.jmatprotec.2007.01.019) resolved to a different PII, so the DOI is unconfirmed | a, snippet | search snippet: "the peak value of cutting forces decreases with the increase of helix angle for a single engaged cutting edge" |
| Woodworking Network / FDMc, *Helical or straight cutting tools?*, https://www.woodworkingnetwork.com/magazine/fdmc-magazine/helical-or-straight-cutting-tools , HTTP 403 | c, snippet | search summary: a helical cutter improves the surface "by redirecting and redistributing cutting forces rather than necessarily reducing them"; as helix rises "axial force rises, radial force stays roughly constant, and tangential force falls" |
| Porankiewicz, Wieczorek, Djurkovic, Idzikowski, Węgrzyn, *Modelling Cutting Forces using the Moduli of Elasticity in Oak Peripheral Milling*, BioResources 16(1):1424-1437 (2021), https://bioresources.cnr.ncsu.edu/wp-content/uploads/2021/01/BioRes_16_1_1424_Porankiewicz_WDIW_Attempt_Cutting_Forces_Modelling_MOE_Oak_Milling_18187.pdf , fetched, read with pdftotext | a | Straight edge only, no spiral arm. "peripheral milling with a straight edge"; "three milling cutters manufactured by Freud (Straight edge cutter, Udine, Italy)"; "diameter (DC = 125 mm), width (WC = 40 mm"; "5860 RPM, i.e., at a constant cutting speed (vC) of 38.35 m/s"; feed per tooth "0.171", "0.341", "0.683" mm; cutting depth "2", "3", "4.5" mm; rake "16", "20", "25" deg. Oak density "<692 kg/m3; 785 kg/m3>". |
| Hobby and retail prose (Canadian Woodworking, Woodsmith, Popular Woodworking, ToolsToday, routerforums), found by search, not fetched | c | Chatter and tear-out claims without numbers. Not used. |

No source read gives a power ratio or a recommended feed adjustment for a
straight bit against a spiral bit. The nearest numbers are the Freud two-table
chip ratio (section 2.1, about 0.46 at 1/2 in) and the Curti Table 5 Ks ratio
(1.1 to 1.9).

## 4. Large cutter on a small shank

Searches run: `router bit 1/4" shank large diameter warning CNC "not
recommended" OR "do not use" depth of cut vendor Whiteside Freud Amana
Infinity`; `Amana Tool straight plunge carbide tipped router bit 3/4" diameter
1/4" shank "Max RPM"`; `Freud router bits "maximum RPM" straight bit 3/4"
diameter 1/4" shank`.

| Finding | Grade | Quote |
|---|---|---|
| Freud (1a) prints a depth rule and a package rule, no shank rule | b | "Never exceed the recommended cutting depth listed on the router bit package or the bit's safety instructions!" and "Always consult your machine's operation manual for bit capacities and recommended feed rates." |
| Vendors sell the combination. Freud 04-140 (3/4 in D, 1/4 in shank) and Amana 45251 (3/4 in D x 1 in CL, 1/4 in shank) are catalogue items with printed maxima (1b, 1i) | c, snippet | see 1b, 1i |
| Whiteside wording differs by shank. The 1/4 in shank bit 3002 says hand-held and table only; the 1/2 in shank bits 1071 and 1072 add CNC | b | 3002: "MACHINES: Designed for use in hand-held routers and router tables in a wide variety of applications." 1072: "Designed for hand-held routers, router tables, and CNC machines" |
| Trend: large cutters table or machining centre only | c, summary | "Cutters over 40mm diameter should normally only be used in a router that is either fixed under a table or overhead in a machining centre." |
| Blog toolsradar.com, *Router Bits Unveiled: 1/4 vs. 1/2 Shanks*, found by search | c, snippet | "a 1/4\" shank has 1/4 the cross-sectional area of a 1/2\" shank, and under heavy cutting loads, the 1/4\" shank flexes"; advises 1/2 in shanks "for hardwoods, deep cuts, or any profile over 3/8\" radius" |
| Forum lumberjocks.com, found by search | c, snippet | one poster: "any bit profile with OD larger than 5/8 inch and with profile cutting depth larger than 3/16 inch; should be used on 1/2 shank" |
| Explicit vendor "do not use in CNC" for an 18-19 mm cutter on a 6 mm or 1/4 in shank | not found | |
| Vendor depth-of-cut or feed limit tied to shank size | not found | |

Shank arithmetic (this report's math, no source): area ratio (6 / 12.7)² =
0.22; bending stiffness ratio (6 / 12.7)⁴ = 0.050; against an 8 mm shank (6 /
8)⁴ = 0.32.

## 5. Power demand or specific cutting energy for oak

| Source | Grade | Value and quote |
|---|---|---|
| Curti 2021 (section 3.1) | a | Oak 737.8 kg/m3 measured; the printed Table 5 fit gives Ks 58.4 N/mm² at 762 kg/m³, GA 90°, down-milling, straight blade (section 3.1 arithmetic). Chip thickness regime 40-100 µm. Forces in N/mm (per mm of axial depth), 20 mm two-flute cutter. |
| *Energy Efficiency and Machined Surface Roughness in Machining Solid Oak Wood (Quercus robur L.) on the High Performance Two-Side Thicknessing Planer*, University of Zagreb repository https://repozitorij.sumfak.unizg.hr/object/sumfak:3789/FILE0/download (HTTP 504 and a dropped socket) and ResearchGate 378307226 (HTTP 403) | a, snippet | search snippet: "the average specific cutting energy was 0.0036 kWh/dm3 of removed wood, specific cutting pressure 13 N/mm2". Check: 0.0036 kWh/dm³ = 12960 J per 10⁶ mm³ = 12.96 N/mm². The two printed numbers agree. Tool: a planer cutter head (straight knives), not a router bit. |
| Porankiewicz 2021 (section 3.2) | a | Measures cutting power and derives force: "PC = PT – PO (W)" and "FA = PCA / vC (N)". Observed forces are in Fig. 5 (image), not in text. Text gives a slope only: "The average change of FCP with an increase of the feed rate per tooth (fZ) by 0.1 mm was between 8 N and 12 N, depending on the value of the rake angle". No N/mm² or kW figure printed. |
| Koleda, Barcík, Korčok, Jamberová, Chayeuski, *Effect of technological parameters on energetic efficiency when planar milling heat-treated oak wood*, BioResources 16(1):515-528 (2021), https://bioresources.cnr.ncsu.edu/resources/effect-of-technological-parameters-on-energetic-efficiency-when-planar-milling-heat-treated-oak-wood/ , fetched | a | Heat-treated Quercus petraea, planing head. Cutting power numbers are in a table image, not in the readable text. "cutting speed (20 m × s-1, 40 m × s-1, and 60 m × s-1), feed rate (6 m × min-1, 10 m × min-1, and 15 m × min-1)". Not usable as a number. |
| *Study of the cutting power in longitudinal milling of oak wood* (ResearchGate 348917186, HTTP 403) | a, snippet | search snippet: "The average cutting power measured during milling sessile oak wood... was 184 W, and the average cutting power during milling pedunculate oak wood was 166 W." Conditions not read. |
| A router-bit (not planer head) specific cutting energy for oak | not found | queries: `specific cutting energy oak milling router bit kJ/cm3 or specific cutting force N/mm2`; `"Energy Efficiency and Machined Surface Roughness in Machining Solid Oak Wood" "specific cutting pressure"` |

Comparison. The planer figure 13 N/mm² and the Curti 58.5 N/mm² differ by a
factor 4.5. The planer value is an energy per removed volume at planer chip
thicknesses (feed 30 m/min class per the snippet); the Curti value is a
marginal coefficient at 40-100 µm chips plus a separate intercept. The chip
thickness regimes differ, so the two numbers are not the same quantity at the
same point. No source read bridges them.

## 6. What this means for the open constants

Facts only; the lead decides.

- **The 14000 tier (`MILLING_CEILING_LARGE_RPM`, tools over 10 mm).** No
  vendor table found prints 14000 as a maximum for an 18-20 mm cutter. Printed
  maxima at 3/4 in: Freud 04-140 24,000 (retailer), Whiteside 18,000, Amana
  45251 35,000 (retailer), CMT flush-trim 19 mm 18,000 (snippet); the UK bands
  put 24,000 (Wealden) or 28,000 (Trend) on every cutter up to 24-25 mm. The
  only 14,000 in any source is the top of Whiteside's recommended range
  "12,000-14,000" for 3/4 in CD bits, which is a recommendation, with 18,000 as
  the printed maximum on the same line. As a cap, 14000 has no source; as a
  recommended operating point it has one vendor, by snippet.
- **The 1.5 speedup (`MAX_SPINDLE_SPEEDUP`) and the 0.95 headroom
  (`SPINDLE_CEILING_HEADROOM`).** No source read states a ratio of chart RPM to
  maximum RPM, or a margin below the spindle maximum. Freud's rule is the
  inequality in 1a: bit rated speed at or above the machine maximum. Nothing
  found supports or contradicts either constant; they stay unsourced.
- **A vendor RPM maximum as a named limit (P21 question 9).** The sources show
  that the maximum is per tool and is printed on the shank (Amana 1h, Trend,
  Wealden, EN 847-1). A diameter table is a retailer or vendor convenience, and
  the bands disagree (24,000 against 28,000 at 24-25 mm). The rim speeds at the
  band edges are 31-47 m/s; the 18 mm tool at 24000 rpm is 22.6 m/s, below
  every band edge read.
- **The chip band in use (Amana Spektra 12.7 mm spiral row, size-scaled).**
  Two vendor tables print a chip for straight carbide-tipped bits at 3/4 in in
  hardwood: Freud .009-.013 in (0.229-0.330 mm) and Onsrud 48-000 .008-.010 in
  (0.203-0.254 mm, column mapping to confirm). Freud's own two tables put the
  straight-bit hardwood chip at about 0.46 of the solid-carbide chip at 1/2 in.
  So a straight-bit row exists at the tool's diameter; the size law from a
  spiral row is not the only option. Whether the straight-bit band is lower or
  higher than the scaled Spektra value is for the calculator to show; this
  report did not run it.
- **The force model at helix 0.** Curti 2021 measured a straight blade as one
  of its three tools and prints a separate fit for it (Table 5). The repo's
  58.5 N/mm² at 762 kg/m³ reproduces the λ=0, GA 90°, down-milling fit. The
  straight blade is the highest-Ks tool of the three (1.1 to 1.9 times the
  helical fits at 762 kg/m³) and the only one with a "non-negligible
  intercept". Helix 0 is inside the paper's data, not an extrapolation. The
  paper's regime is 3000 rpm, 20 mm cutter, 40-100 µm chips; the operator's
  chips (section 2) are 2 to 3 times thicker, which is outside the measured h
  range.
- **Power check.** The one oak energy figure found (13 N/mm², planer head,
  snippet) is below the Curti coefficient by 4.5x and is at a thicker chip.
  It cannot confirm or deny the Curti value at the operator's chip without the
  paper's conditions, which the fetch did not reach.
- **Shank.** No vendor limit ties depth, feed or RPM to a 6 mm shank under an
  18 mm cutter. Vendors sell 3/4 in cutters on 1/4 in shanks with printed
  maxima of 24,000 and 35,000. Whiteside labels its 1/4 in shank bit for
  hand-held and table use and its 1/2 in shank straight bits for CNC as well;
  that is wording, not a stated rule.

## 7. Fetch log

Fetched and read: Freud 2017 PDF (pdftotext); Curti 2021 PDF (pdftotext);
Porankiewicz 2021 PDF (pdftotext); Whiteside collection page, 1072, 1071, 3002;
Wealden cutter-speeds and cutters-use; Trend terminology (first 40k chars
verbatim, rest summarised); Koleda 2021 BioResources page.
Refused or failed: scosarg CMT leaflet (403); woodworkingnetwork (403);
ResearchGate x3 (403); amanatool.com maxrpm (403); toolstoday maxrpm x2 (404);
Leitz Lexicon 05 (over 10 MB); ScienceDirect 2007 (403) and its DOI (wrong
PII); i3detroit Bosch manual (bot wall); Zagreb repository PDF (504, then
dropped socket, two tries).
Search queries: 13, listed inline where a gap is reported.
