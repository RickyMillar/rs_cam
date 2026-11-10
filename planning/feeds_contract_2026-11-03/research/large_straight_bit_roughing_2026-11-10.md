# 18 mm straight bit as a roughing tool in white oak: published evidence (2026-11-10, round two)

Research report for the feeds contract. Read-only on the code. Access date for
every web source: 2026-11-10. Grades: `a` = standard or peer-reviewed, `b` =
vendor table or vendor catalogue, `c` = vendor prose, retailer page or forum.
A row marked **snippet** was read in a web-search result only. This report
gives facts and arithmetic. It gives no recommendation about what to cut.

Round one is `large_straight_router_bits_2026-11-10.md` in this folder. This
report does not repeat it.

The question: an 18 mm, 2-flute, straight-flute bit, 20 mm flute, 6 mm shank,
in white oak on a Shapeoko Pro XXL with a 1.5 kW spindle. Depth of cut 10 mm,
width of cut about 6 mm, feed 0.18-0.20 mm per tooth, 18000-24000 rpm. The
app's force line is Curti 2021: `F/b = 58.5 h + 4.6` N per mm of edge at
762 kg/m3.

All PDFs were read locally with `pdftotext -layout`. Numbers in a figure
(image) are not in the text and are not quoted.

## 0. A geometry fact that changes the question

Curti 2021 states its chip range as an average uncut chip thickness, not as a
feed per tooth. Round one quotes Table 3: "Spindle speed 3000 rpm", "Feed rate
2000 mm/min", 2 flutes, 20 mm cutter, "Radial depth of cut 0.5, 1.0, 1.5, 2.0,
2.5 mm", and "average uncut chip thicknesses h roughly varying from 40 µm ...
to 100 µm".

Arithmetic (this report). Feed per tooth in Curti: 2000 / (3000 x 2) =
0.333 mm. Engagement angle: `phi = acos(1 - ae/R)`. Largest chip:
`h_max = fz sin(phi)`. Average chip: `h_avg = fz ae / (R phi)`.

| Case | fz, mm | ae, mm | phi, deg | h_max, mm | h_avg, mm |
|---|---|---|---|---|---|
| Curti, 20 mm cutter | 0.333 | 0.5 | 18.2 | 0.104 | 0.053 |
| Curti, 20 mm cutter | 0.333 | 1.5 | 31.8 | 0.176 | 0.090 |
| Curti, 20 mm cutter | 0.333 | 2.5 | 41.4 | 0.221 | 0.115 |
| Operator, 18 mm cutter | 0.18 | 6 | 70.5 | 0.170 | 0.098 |
| Operator, 18 mm cutter | 0.19 | 6 | 70.5 | 0.179 | 0.103 |
| Operator, 18 mm cutter | 0.20 | 6 | 70.5 | 0.189 | 0.108 |

The computed Curti averages (0.053 to 0.115 mm) agree with the paper's "roughly"
40 to 100 µm. So:

- Curti's feed per tooth (0.333 mm) is larger than the operator's (0.18-0.20 mm).
- The operator's average chip (0.098-0.108 mm) is at the top edge of Curti's
  average-chip range (about 0.10-0.115 mm).
- The operator's largest chip (0.170-0.189 mm) is below Curti's largest chip at
  ae 2.5 mm (0.221 mm).
- The operator's engagement angle (70.5 deg) is outside Curti's (18 to 41 deg).

The statement "the operator's chips are about twice Curti's" compares a feed
per tooth with an average chip thickness. This report did not read how the
app converts feed per tooth to `h`. If the app puts fz = 0.19 mm into the line
as `h`, the result is `58.5 x 0.19 + 4.6 = 15.7 N/mm`. If the app uses the
average chip, the result is `58.5 x 0.103 + 4.6 = 10.6 N/mm`.

## 1. Specific cutting force against chip thickness

### 1.1 Answer

Every source read gives the same form: the cutting force per mm of edge is
linear in chip thickness with a positive intercept. So the specific force
(force / chip area) falls as the chip gets thicker, and it approaches the
slope. No source read shows a rise. The best source for thick chips in white
oak is Woodson 1979 (grade a, USDA). Its slope for white oak (57.6 N/mm2 at
20 deg rake) is within 2 % of the Curti coefficient (58.5 N/mm2), from chips 4
to 15 times thicker than Curti's.

### 1.2 Woodson 1979: white oak, chips 0.38 to 1.52 mm

Source: G. E. Woodson, *Tool Forces and Chip Types in Orthogonal Cutting of
Southern Hardwoods*, USDA Forest Service Research Paper SO-146 (1979),
Southern Forest Experiment Station.
https://research.fs.usda.gov/download/treesearch/599.pdf (record:
https://research.fs.usda.gov/treesearch/599 ). Fetched, 2.3 MB. Grade a
(government research paper).

Quoted method: "Specimens (1/8 to 1/4 inch thick) from 5 trees of each of 22
hardwood species were cut orthogonally at 5 inches per minute." "four depths of
cut (.015, .030, .045, and .060 inch)". Rake angles "10, 20, and 30 degrees for
planing; 20, 30, and 40 degrees for crosscutting". "Clearance angle 15°".
Table 1: "White oak (Quercus alba L.) ... 0.644" (specific gravity, "ovendry
weight and green volume"). Summary: "Average parallel forces generally
increased with deeper cuts and greater specific gravity but decreased with
greater rake angle and moisture content."

Table 18B, "TOOL FORCES WHEN ORTHOGONALLY CUTTING WHITE OAK PARALLEL TO THE
GRAIN IN THE 90-0 DIRECTION", parallel force, moisture content 10.9 %,
"pounds per 0.1 inch of knife", average value:

| Depth of cut, in | Rake 10 | Rake 20 | Rake 30 |
|---|---|---|---|
| 0.015 | 19.3 | 16.2 | 5.4 |
| 0.030 | 33.3 | 31.2 | 10.4 |
| 0.045 | 41.0 | 40.8 | not legible in the text layer |
| 0.060 | 50.2 | 54.8 | 12.7 |

Table 18C, "... ACROSS THE GRAIN IN THE 90-90 DIRECTION", moisture content
11.2 %: rake 20: 24.6, 39.6, 48.7, 54.6. Rake 30: 20.9, 31.6, 43.1, 48.0.

Conversion (this report): 1 lbf per 0.1 in = 4.4482 N / 2.54 mm = 1.7513 N/mm.
Depths: 0.381, 0.762, 1.143, 1.524 mm.

| Direction, rake | h, mm | F/b, N/mm | F/(b h), N/mm2 |
|---|---|---|---|
| 90-0, 20 deg | 0.381 | 28.37 | 74.5 |
| 90-0, 20 deg | 0.762 | 54.64 | 71.7 |
| 90-0, 20 deg | 1.143 | 71.45 | 62.5 |
| 90-0, 20 deg | 1.524 | 95.97 | 63.0 |
| 90-0, 10 deg | 0.381 | 33.80 | 88.7 |
| 90-0, 10 deg | 1.524 | 87.91 | 57.7 |
| 90-0, 30 deg | 0.381 | 9.46 | 24.8 |
| 90-0, 30 deg | 1.524 | 22.24 | 14.6 |
| 90-90, 20 deg | 0.381 | 43.08 | 113.1 |
| 90-90, 20 deg | 1.524 | 95.62 | 62.7 |
| 90-90, 30 deg | 0.381 | 36.60 | 96.1 |
| 90-90, 30 deg | 1.524 | 84.06 | 55.2 |

Least-squares line through the four points of each row (this report's fit):

| Direction, rake | Slope, N/mm2 | Intercept, N/mm |
|---|---|---|
| 90-0, 20 deg | 57.6 | 7.71 |
| 90-0, 10 deg | 46.1 | 19.00 |
| 90-0, 30 deg (3 points) | 10.3 | 7.44 |
| 90-90, 20 deg | 45.6 | 29.95 |
| 90-90, 30 deg | 42.7 | 22.24 |

The specific force falls with chip thickness in every row.

Against Curti (rake 25 deg, 762 kg/m3, `58.5 h + 4.6`):

| h, mm | Curti line, N/mm | Curti F/(b h), N/mm2 | Woodson 90-0 rake 20, N/mm | Note |
|---|---|---|---|---|
| 0.07 | 8.70 | 124.2 | | inside Curti's range |
| 0.10 | 10.45 | 104.5 | | top of Curti's range |
| 0.19 | 15.71 | 82.7 | 18.66 (fit line, below Woodson's thinnest cut) | 57.6 x 0.19 + 7.71 |
| 0.381 | 26.89 | 70.6 | 28.37 (measured) | Curti line extended 3.8x past its range |

At 0.381 mm the extended Curti line (26.89 N/mm) is 5 % below Woodson's
measured value at 20 deg rake (28.37 N/mm) and 2.8 times Woodson's value at
30 deg rake (9.46 N/mm). Curti's rake (25 deg) is between the two. At 0.19 mm,
Woodson's 20 deg fit gives 18.66 N/mm, which is 1.19 times the Curti value
(18.66 / 15.71).

Limits of this comparison. Woodson cut at 5 inches per minute with a
hand-honed knife; Curti milled at 3000 rpm. Woodson's 0.644 is a specific
gravity on green volume; Curti's 762 is an air-dry density. The 90-0 rake 30
row has a different chip type; Woodson: "Cantilever-type failure occurred in
all species at high rake angles and low moisture content."

Woodson also prints a species regression. Figure 7, "planing dry wood (10
percent moisture content) of 22 species with a 10° rake angle", average
parallel force = y-intercept + slope x specific gravity, lb per 0.1 in:
depth .015: "-2.24", "33.79"; .030: "-3.20", "61.68"; .045: "7.55", "59.38";
.060: "10.40", "73.26". At specific gravity 0.644 this gives 19.52, 36.52,
45.79, 57.58 lb per 0.1 in = 34.2, 64.0, 80.2, 100.8 N/mm = 89.7, 83.9, 70.2,
66.2 N/mm2. The specific force falls here also.

### 1.3 Hlásková et al. 2015: beech, linear with an intercept

Source: M. Hlásková, K. A. Orlowski, Z. Kopecký, M. Jedinák, *Sawing Processes
as a Way of Determining Fracture Toughness and Shear Yield Stresses of Wood*,
BioResources 10(3):5381-5394 (2015),
https://bioresources.cnr.ncsu.edu/wp-content/uploads/2016/06/BioRes_10_3_5381_Hlaskova_OKJ_Sawing_Processes_Way_Determining_Fracture_7390.pdf
. Fetched. Grade a.

Quotes: "Beech wood (Fagus sylvatica L.) ... a density of 691 kg/m3 ...
conditioned to 8% to 12% moisture content". "Both cutting force trends were
linear". Sash gang saw, "kerf width (St) 2 mm": "Fc1 = 328060 fz + 7.77 (N)"
(fz in m). Circular saw, "kerf width (St) 3.5 mm": "Fc1 = 301816 h + 3.792
(N)". "calculations ... were conducted for uncut chip thicknesses h > 0.12 mm,
when the cutting resistance is practically constant (Orlowski 2003)". The
figures show feed per tooth up to about 0.18 mm (gang saw) and chip thickness
up to about 0.14 mm (circular saw). Gang-saw edge radius "around 55 µm",
"not the normal 'sharp' tool".

Arithmetic (this report), per mm of kerf:

| Machine | Slope, N/mm2 | Intercept, N/mm | F/b at 0.07 | F/b at 0.19 | F/(b h) at 0.07 | at 0.19 |
|---|---|---|---|---|---|---|
| Gang saw (328060/1000/2; 7.77/2) | 164.0 | 3.89 | 15.37 | 35.05 | 219.5 | 184.5 |
| Circular saw (301816/1000/3.5; 3.792/3.5) | 86.2 | 1.08 | 7.12 | 17.47 | 101.7 | 91.9 |

These are rip-sawing cuts (the edge cuts across the fibres) with a closed
kerf, so the levels are not the same quantity as a peripheral milling force.
The form is the same as Curti's: a line with an intercept. The circular-saw
value at 0.19 mm (17.47 N/mm) is 1.11 times the Curti value (15.71 N/mm);
0.19 mm is past the circular-saw data (about 0.14 mm).

### 1.4 Mandić, Porankiewicz, Danon 2015 and Porankiewicz 2021: oak, thin average chips

Source: M. Mandić, B. Porankiewicz, G. Danon, *An Attempt at Cutting Forces
Modelling in Oak Peripheral Milling*, BioResources 10(3):5489-5502 (2015),
https://bioresources.cnr.ncsu.edu/wp-content/uploads/2016/06/BioRes_10_3_5489_Mandic_PD_Attempt_Model_Cutting_Forces_Oak_6812-1.pdf
. Fetched. Grade a.

Quotes: "Three cutters were used of diameter 125 mm, and a width of 40 mm,
equipped with four soldered plates, made of cemented carbide H302 manufactured
by Freud". "density, D values were within the range of 613 kg·m-3 to 790
kg·m-3". Table 1: feed per tooth "0.171", "0.341", "0.683" mm; cutting depth
"2", "3", "4.5" mm. Table 2, "Mean cutting layer thickness, aP (mm)": 0.022,
0.026, 0.032 at fz 0.171; 0.043, 0.053, 0.063 at fz 0.341; 0.086, 0.106,
"0.13" at fz 0.683.

So this study (and Porankiewicz 2021, same cutters and feeds, round one
section 3.2) has feeds per tooth of 0.17 to 0.68 mm but average chips of 0.022
to 0.13 mm. The fitted model is a 17-term equation with material properties;
the paper warns that it "can be valid only for ranges of independent variables
chosen within the experimental matrix". This report did not evaluate it. It
gives no measured point at an average chip above 0.13 mm.

The paper also records the Kivimaa-based law used by Ettelt and Gittel (2004),
Tröger and Scholz (2005) and Scholz et al. (2009): "kC0.5 = FC · aP^-1/2" with
"kC - is the specific cutting resistance (N·mm-1/2)". That law makes force
proportional to the square root of chip thickness, so the specific force falls
as h^-0.5. The authors add: "it is difficult to find justification for the use
of the following equations". No coefficient for oak is printed.

### 1.5 Cristóvão 2013 (Luleå): dense tropical woods, chips 0.15 to 1.0 mm

Source: L. Cristóvão, *Machining Properties of Wood: Tool Wear, Cutting Force
and Tensioning of Blades*, doctoral thesis, Luleå University of Technology
(2013), http://www.diva-portal.org/smash/get/diva2:999250/FULLTEXT01.pdf .
Fetched, 8.3 MB. Grade a.

Quotes: Table 2, chip thickness levels "0.15", "0.50", "1.00" (mm); rake "10",
"20", "30"; edge radius "5", "25", "50" (µm). "FP Ntholo = −6.55 − 0.08·α +
0.18·r + 0.03·D + 28.94·t + 0.10·MC − 4.22·CD (Nmm-1)". "FP Iron wood = 8.16 −
0.18·α + 0.17·r + 0.01·D + 31.21·t − 0.10·MC − 4.37·CD (Nmm-1)". "There is no
non-linear model in this article." "it has been pointed out by Grönlund (2004)
that low values of chip thickness greatly increase the specific cutting work."

The fit is linear in chip thickness with a slope of 28.9 to 31.2 N/mm2 across
0.15 to 1.0 mm, averaged over three cutting directions. The species are not
oak. This report did not evaluate the equations (the densities of the two
species were not read).

### 1.6 Goli and co-workers 2018: same method as Curti, same form

Source: G. Goli et al., *Specific Cutting Forces of Isotropic and Orthotropic
Engineered Wood Products by Round Shape Machining*, Materials 11(12):2575
(2018), https://pmc.ncbi.nlm.nih.gov/articles/PMC6315737/ . Fetched. Grade a.

Quotes: "average chip thicknesses (hm) of 0.041, 0.062, 0.078 and 0.091 mm".
"they were plotted against chip thickness and a linear regression was applied
in order to calculate the slope and the intercept ... The slope corresponds to
the specific cutting coefficient Ks." Same chip range as Curti; no thicker
chip.

### 1.7 Not found or not read

| Target | Result |
|---|---|
| Kivimaa 1950, *Cutting Force in Woodworking*, VTT Publication 18 | No full text online. Google Books and Open Library records only. Secondary quote (search snippet, grade c): cross-cutting "9 kp/cm (chip thickness 0.1 mm)", ripping "3.5 kp/cm", species birch. Not used. |
| Koch, *Wood Machining Processes* (1964) | Not found online. Woodson 1979 is the same laboratory's hardwood data set. |
| Porankiewicz and Goli 2014, *Cutting forces by Oak and Douglas fir machining*, Maderas 16(2):199-216 | flore.unifi.it HTTP 403; revistas.ubiobio.cl no connection; scielo.cl HTTP 403. Not read. |
| Axelsson, Lundberg, Grönlund 1993 | Not searched directly. Cited inside Mandić 2015 and Cristóvão 2013 only. |
| Naylor and Hackney; Aguilera and Martin; McKenzie 1961; Ettelt and Gittel | Not fetched. Named as citations inside the papers above. |
| FPL Wood Handbook, machining chapter | Not searched (budget). |
| Orlowski and Ochrymiuk 2017, Maderas 19(2):149-162 (fetched, https://www.redalyc.org/pdf/485/48550384003.pdf ) | Pine only. Same fracture-mechanics line as 1.3. Not used. |
| Đukić et al. 2026, BioResources 21(3):7395-7411 (fetched) | Oak planing power; numbers are in figures. Not used. |

Searches: `specific cutting force wood milling chip thickness dependence oak
beech "specific cutting resistance" ... Kivimaa Orlowski`; `Woodson 1979 "Tool
forces and chip types in orthogonal machining of southern hardwoods"`;
`Kivimaa 1950 "Cutting force in wood-working" birch ... table`; `Porankiewicz
Goli 2014 ...`; `Hlásková Orlowski Kopecký ...`; `Porankiewicz oak peripheral
milling ... 0.171 0.341 0.683`.

## 2. Pages left unread in round one

| Item | Result | Grade |
|---|---|---|
| Onsrud 48-000 chart, on the PDF itself | LMT Onsrud, *Production Cutting Tools* catalogue PCT-19, https://onsrud.com/images/LMT%20Onsrud%20Product%20Cutting%20Tools%20Catalog%20PCT-19.pdf , fetched (20.9 MB). "Hard Wood Cutting Data Recommendations", row "48-000 1xD". A script matched each cell to the nearest header by character position: 3/16 ".004-.006"; 1/4, 5/16, 3/8 ".005-.007"; 1/2 ".006-.008"; 5/8 ".007-.009"; 3/4 ".008-.010"; 7/8 ".009-.011"; 1 ".010-.012". The round-one mapping is confirmed. 3/4 in: 0.203-0.254 mm per tooth. Depth rule on the same chart: "DEPTH OF CUT: 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%". | b |
| Onsrud 48-000 series heading | Same catalogue: "48-000 Series Two Flute - Carbide Tipped Straight Product Offering"; "Designed for general usage where superior balance and vibration free cutting provides a smoother finish along with long tool life." Row "48-016  3/4  1  1/4  2-1/4  2" (cutting diameter, length of cut, shank diameter, overall length, flutes). Onsrud lists a 3/4 in two-flute straight bit on a 1/4 in shank with a 1 in cut length. | b |
| Whiteside 3/4 in straight bit, RPM line | whitesiderouterbits.com/products/1030 returned HTTP 429. pmctool.com returned HTTP 429. The taytools.com page was fetched but its text holds no RPM line (script-rendered table). Search result for taytools.com and pmctool.com (Whiteside 1030, 3/4 in CD, 3/4 in CL, 1/4 in shank): recommended 12,000-14,000 RPM, maximum 18,000; Tay Tools text says the bit is for router tables and CNC machines, not hand-held routers. Still a snippet. | b, snippet |
| Leitz Lexicon, routing chapter | *Leitz Lexicon Edition 7 - 05 Routing*, https://www.leitz.org/fileadmin/Downloads/Lexicon/EN/Leitz_Lexicon_Edition_7_-_05_Routing.pdf , fetched (11.4 MB). "Grooving cutter, Z 2 ... Straight cut, tungsten carbide plunging tip. HW, Z 2, shank 9.5 / 12 mm": rows "16 90 35 12x40", "18 90 35 12x40", "20 90 35 12x40" (D, GL, NL, shank). Feed diagram notes: "Operation: Grooving / Speed: n = 18000 min-1 / Correction factor for vf: Solid wood = 0.8; Glulam = 0.8; Machining across grain = 0.7". The feed curves (vf against ap) are images; no value is readable. No tooth-feed table for solid wood is in the text of this chapter. A pattern search found straight grooving cutters on a 6 mm shank only at D 3 to 6 mm; two profile cutters with D 16.7 and 18.7 mm and one 21 mm trimming cutter have a "6x30" shank. | b |
| Leitz tooth feed for solid wood | Search summary of *Leitz Lexicon Edition 7 - 04 Manual feed*: tooth feed 0.60-0.80 mm along the grain, 0.30-0.40 mm across the grain. Not fetched. | b, snippet |
| EN 847-1 speed marking rule, free summary | Not found. Search results show contents only: EN 847-1:2013 has "6.2 Marking of milling tools for machines with hand feed", a "Maximum speed" clause (7.2.1) and "A.4 Milling tools marked with MAN". EN 847-2 is *Requirements for the shank of shank mounted milling tools*; its scope snippet: "determination of the maximum speed for given eccentricity at clamping devices for the shank strength of milling tools with cylindrical and taper shank"; it "applies also for shank tools with a cutting diameter of less than 16 mm"; "the minimum value for the clamping length l shall be as given in Table 2". No table value was read. Query: `EN 847-1 milling tools maximum rotational speed marking "n max" shank tools "MAN" "MEC" ...`. | a, snippet |

## 3. Depth of cut per pass and shank grip: published statements

| Source | Grade | Quoted line |
|---|---|---|
| Trend Tool Technology, *Feeding the Material* (router basics), https://www.trend-usa.com/router-basics-feeding-the-material , fetched | c (vendor prose) | "As a rough guide the depth of cut should not exceed the diameter of the cutter when you are using cutters up to 1/2” diameter, or the diameter of the cutter shank, whichever is smaller." "If any of the criteria cannot be met, take several shallower passes until the full depth of cut is reached." The guide's stated conditions: "The cutter has a standard shank length. The cutting length of the cutter is not excessive relative to the depth of cut. The workpiece is of medium density timber. A feed speed of 2-3 metres/minute is maintained. The cutting edges are clean and sharp. At least three quarters of the cutter shank is gripped in the collet." "Never force a cutter to rout too deep a cut in one pass". "When using large moulding or panelling cutters, the full depth of cut must be reached in a series of shallow steps." The page is for hand-held routers. |
| LMT Onsrud PCT-19 (section 2) | b | "DEPTH OF CUT: 1 x D Use recommended chip load"; "To ensure proper clamping, the tool shank should fill, at the minimum, 80% of the depth of the collet."; "make sure the flute fadeout does not enter the collet". |
| Freud 2017 sheet (round one, 1a) | b | "Recommended* Chip Loads, based on cut depth equal to bit diameter" (solid carbide table); "Never exceed the recommended cutting depth listed on the router bit package or the bit's safety instructions!" |
| Ryobi R161 and Craftsman 315.175040 router manuals, manualslib.com (HTTP 403 to fetch) | c, snippet | "we recommend that you do not exceed 1/8 inch depth of cut in a single pass, regardless of the bit size or the softness or condition of the workpiece"; router cuts "can be made deeper in soft woods, such as white pine, than in tough hardwoods like oak or maple". Hand-held routers. |
| McFeely's listing for a CMT 1/4 in straight bit | c, snippet | "For best results with small-diameter bits, make deep cuts in multiple passes rather than attempting to remove excessive material in a single pass." |
| Forum (sawdustzone.org), a rule attributed to Pat Warner | c, snippet | not cutting more than "the equivalent of 1/4\" x 1/4\"" per pass. |
| Leitz Lexicon Ed. 7, slot cutting note | b | "Reducing the slot cutting width by approx 0.1 mm per stroke reduces the risk of breakage as the tool does not touch the side of the slot with the full length of the tool." (for a mortise cutter family, not this bit) |
| A vendor depth limit for a 3/4 in straight bit on a 1/4 in shank | not found | Queries: `router bit depth of cut per pass straight bit "do not exceed" diameter hardwood "multiple passes" 1/4" shank manufacturer instructions`; `router bit manufacturer safety instructions "depth of cut" "passes" ... Freud OR CMT OR Amana OR Trend OR Infinity`; `CMT router bits instructions "1/4" shank" "maximum depth" ...`; `Trend routing guide "depth of cut" "should not exceed" ...`. |

Arithmetic against the operator's cut (this report). Trend's rough guide for
a 6 mm shank gives 6 mm; the operator's depth is 10 mm, which is 1.67 times
that. Trend states the cutter-diameter part of the guide only "up to 1/2”
diameter"; the bit is 18 mm. Onsrud's and Freud's 1 x D reference depth is
18 mm; 10 mm is 0.56 x D.

## 4. Shank bending

| Item | Result |
|---|---|
| Published allowable bending stress, fatigue limit or maximum side load for a 6 mm or 1/4 in router-bit shank | **Not found.** No vendor page, catalogue or paper read gives one. |
| A standard that covers shank strength | EN 847-2 (section 2) covers shank strength and a maximum speed against clamping eccentricity. The text was not read; it is a paid standard. |
| Shank steel grade stated by a bit maker | Not found from a maker. Retailer statements only (grade c, snippets): Axminster, Axcaliber 1/4 in shank cutter: "SAE4140 steel shank hardened to Rockwell 45 HRC". Busy Bee Tools: "All bits are manufactured from 4140 & 5140 steel and are put through a special heat treatment process". JB Tools, Grizzly bit: "SAE4140 steel body, heat-treated". Freud, Whiteside, Amana, Onsrud, CMT: no grade found. |
| Vendors that sell the geometry | Onsrud 48-016 (3/4 in D, 1 in cut, 1/4 in shank, section 2); Freud 04-140 and Amana 45251 (round one); Whiteside 1030 (snippet). |
| Leitz practice | Leitz's 16, 18 and 20 mm straight grooving cutters have a 12 x 40 mm shank (section 2). |

Query: `router bit shank steel grade "4140" OR "SAE 4140" OR "42CrMo4" OR
"Fatigue Proof" carbide tipped router bit body material manufacturer states`.

Facts a calculation needs, and their status:

1. Shank steel grade and hardness: retailer statements only (4140, 45 HRC for
   one brand); not known for the operator's bit.
2. Fatigue strength of that steel at that hardness under rotating bending: not
   searched; no router-bit source gives it.
3. The stress concentration where the shank meets the body, and the braze
   position: not found.
4. The overhang from the collet face to the load, and the grip length: set by
   the operator; Onsrud states 80 % of the collet depth, Trend three quarters
   of the shank.
5. The load: the app's force line (sections 0 and 1), with the radial part
   unknown for this tool. Curti's Table 4 gives only the axial ratio for a
   straight blade in oak ("0.0% (2.2%)").
6. The load cycle: with a straight edge and a 70.5 deg engagement, each tooth
   loads the shank once per turn; Curti notes "the high dynamic excitation
   when machining with the straight blade tool".

This report does not estimate a shank limit.

## 5. Published experience at this scale

All grade c (Carbide 3D community forum, read as thread JSON; the posters are
users unless marked staff).

| Thread | Statement |
|---|---|
| *Surfacing Bits for Oak(hardwoods)*, 2021-05-31, https://community.carbide3d.com/t/surfacing-bits-for-oak-hardwoods/33382 | Question from a Shapeoko XXL owner who works "mainly ... in oak (white)". Staff reply: "I'm cheap and patient just use a #201 or #251 — some folks use larger tooling ... you want to take a pretty light cut." User: "I use a whiteside 6120 1 inch surfacing bit. I have it set up for 18k rpm, 0.01 inch doc and 80 ipm." A posted tool-library row for the same bit: feed 80, rpm 16000, depth 0.01 (inch units), material MDF. |
| *Speed and Feed for 1" Face Mill*, 2019-02-09, https://community.carbide3d.com/t/speed-and-feed-for-1-face-mill/12777 | User: "I use 10K RPM for a 1 1/4” surfacing bit on hardwood. DOC, step over and feed rate will depend upon what you are machining." |
| *Larger bits for roughing?*, 2020-09-08, https://community.carbide3d.com/t/larger-bits-for-roughing/25675 | An XXL owner asks if a larger bit shortens roughing in wood and MDF. Replies name a 1 in surfacing bit and a "more aggressive 1/4\" bit". No reply gives a feed or depth for a large bit used as a roughing tool. Staff offer to optimise the project file instead. |
| Search summaries of other threads (not fetched) | 1 in surfacing bit at "100ipm, at .0625 depth of cut w/40% stepover, and roughly 12000rpm"; Freud 1.25 in mortising bit in oak-family woods at ".015 depth of cut, 25% step over, 100 inches/min"; for oak one user "rarely" cuts "more than 2mm per pass". |

Arithmetic (this report). The deepest reported cut is 0.0625 in = 1.59 mm at
100 ipm = 2540 mm/min. The lightest is 0.01 in = 0.25 mm at 80 ipm = 2032
mm/min; with 3 flutes at 18000 rpm that is 2032 / (18000 x 3) = 0.038 mm per
tooth. The operator's case is 10 mm deep at 7000-9300 mm/min.

Not found: a vendor application note, a vendor engineer's answer or a
university shop guide that states feeds and depths for roughing hardwood with
a 3/4 in straight bit on a hobby gantry. Query: `Shapeoko 1 inch surfacing bit
hardwood oak feeds speeds depth of cut Carbide 3D McFly recommended feed rate
1/4" shank`. A Carbide 3D chart for its own surfacing cutter was not found by
that search.

## 6. What this means for the 18 mm bit

Facts only.

**The force at this chip.**

- Supports the form. Woodson 1979 (white oak, 0.38-1.52 mm), Hlásková 2015
  (beech, to about 0.14-0.18 mm), Cristóvão 2013 (0.15-1.0 mm) and Goli 2018
  all fit force per mm of edge as a line in chip thickness with a positive
  intercept. The Curti line has that form.
- Supports the slope. Woodson's white oak slope at 20 deg rake, planing
  direction, is 57.6 N/mm2 against the app's 58.5 N/mm2. At 0.381 mm the
  extended Curti line is 5 % below Woodson's measured 20 deg value.
- Leaves the level open by rake and direction. Woodson's intercept at 20 deg
  rake is 7.71 N/mm against the app's 4.6 N/mm; at 0.19 mm the Woodson fit is
  1.19 times the Curti line. Woodson's 30 deg rake value at 0.381 mm is 0.35
  times the extended Curti line. Woodson's 90-90 rows (edge across the fibres)
  have intercepts of 22 to 30 N/mm. The operator's bit rake angle is not
  known.
- Contradicts nothing. No source read shows the specific force rising with
  chip thickness above 0.1 mm. One cited law (Kivimaa-based, h^0.5) makes it
  fall faster than a line with an intercept; no oak coefficient was read.
- Section 0. On average chip thickness, the operator's cut (0.098-0.108 mm) is
  at the top edge of Curti's range, not at twice the range. On feed per tooth,
  Curti's 0.333 mm is above the operator's 0.18-0.20 mm. The engagement angle
  (70.5 deg against 18-41 deg) is the input that is outside Curti's tests.

**The chip band.** Onsrud's 48-000 hard-wood row at 3/4 in is confirmed on the
PDF: .008-.010 in (0.203-0.254 mm) at 1 x D. With Freud's .009-.013 in (round
one), two vendor tables put a 3/4 in carbide-tipped straight bit in hardwood
at or above the operator's 0.18-0.20 mm. Leitz's solid-wood tooth feeds are
larger still (snippet). No source read puts the band below 0.18 mm for this
diameter.

**The depth of cut.** The vendor charts state their chip at a depth of 1 x D
(18 mm); the operator's 10 mm is inside that. One vendor guide for hand-held
routers (Trend, prose) limits depth to the shank diameter when that is the
smaller dimension: 6 mm against the operator's 10 mm. Router manuals state
1/8 in per pass (snippet). No vendor limit specific to a 3/4 in bit on a
1/4 in shank was found.

**The shank.** No published allowable stress, fatigue value or side-load limit
exists in what was searched. EN 847-2 covers shank strength; its text was not
read. The steel grade is known only from retailers of other brands (4140, 45
HRC). Leitz puts its 18 mm straight cutter on a 12 mm shank; Onsrud, Freud,
Amana and Whiteside sell 3/4 in cutters on 1/4 in shanks.

**Experience.** Every hobby-gantry report found uses a large cutter for
surfacing at 0.25 to 1.6 mm depth. No report found describes a 3/4 in bit at
10 mm depth on this class of machine, with success or with failure.

**Still unknown.**

1. How the app converts feed per tooth to `h` in the force line (section 0).
2. The rake angle of the operator's bit, which moves the Woodson level by a
   factor of about 3 between 20 and 30 deg.
3. Any measured milling force in oak at an average chip above 0.13 mm or at an
   engagement angle near 70 deg.
4. The radial (normal) force on a straight edge at this engagement. Woodson's
   normal force for white oak at 20 deg rake, 0.381 mm is "1.9" lb per 0.1 in
   (3.3 N/mm); no milling value was read.
5. A shank limit, the shank steel, and the EN 847-2 rule.
6. The Whiteside 1030 RPM line on the maker's own page (HTTP 429).
7. Kivimaa 1950, Koch 1964, Porankiewicz and Goli 2014: text not reached.

## 7. Fetch log

Fetched and read (17): Porankiewicz 2021 PDF (again, for cross-check); Mandić
2015 PDF; Đukić 2026 PDF; Cristóvão 2013 thesis; Orlowski and Ochrymiuk 2017
PDF; Hlásková 2015 PDF; Hlásková 2018 PDF (not used); Goli 2018 (PMC);
Treesearch record 599; Woodson 1979 PDF; Onsrud PCT-19 PDF; three Carbide 3D
threads; Trend *Feeding the Material*; Leitz Lexicon Ed. 7 chapter 05; Leitz
2011 chapter 05 (not used); taytools.com page (no RPM text).
Refused or failed (9): mdpi.com (403); manualslib.com (403); pmctool.com (429);
whitesiderouterbits.com/products/1030 (429); srs.fs.usda.gov (host not
resolved; the Treesearch copy worked); flore.unifi.it (403); revistas.ubiobio.cl
(no connection); scielo.cl (403); first Luleå attempt (time-out, second try
worked).
Search queries: 17, listed inline where a gap is reported.
