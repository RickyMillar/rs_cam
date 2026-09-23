# Machinist reference check — published chipload and finishing data

Date: 2026-09-19.
Author: a research agent. The agent read no file in this repository.
Every number below comes from the open web. Every number carries a URL.

## How to read this file

- A "vendor" source is a tooling manufacturer. A "CAM vendor" source is a
  software publisher. A "retailer" source sells tools but does not make them.
- The agent quotes at most about 15 words of prose from each page.
- The agent reproduces numeric table rows as facts.
- The agent marks its own arithmetic as "derived". The agent does not present
  derived numbers as published numbers.
- **Caution: read the conditions line of each table before you use a row.**
  Most wood charts state one condition: the axial depth of cut equals one
  tool diameter. A light finishing stepover is a different case.

---

## 1. Chipload charts for CNC routers in wood

### 1.1 LMT Onsrud — "Cutting Data Recommendations"

Onsrud publishes one table per material. Each table gives the recommended
chip load per tooth by cutting diameter. Each table states the same depth
rule:

> DEPTH OF CUT: 1 x D Use recommended chip load / 2 x D Reduce chip load by
> 25% / 3 x D Reduce chip load by 50%

Index page:
https://onsrud.com/Forms/Cutting-Data-Recommendations.asp

Note: Onsrud tabulates 1/16 in for the 64-000/65-000 series in all five wood
tables, and for the 10-00 series in the Soft Wood table only. No other
Onsrud series has a 1/16 in row in any wood table.

#### Hard Wood (https://onsrud.com/images/Hard%20Wood.pdf, page 115)

| Series | 1/16 | 1/8 | 3/16 | 1/4 |
|---|---|---|---|---|
| 40-000 | — | .006-.008 | .007-.009 | .008-.010 |
| 40-100 | — | .004-.006 | .005-.007 | .005-.007 |
| 52-200 / 57-200 | — | .003-.005 | .004-.006 | .005-.007 |
| 52-700 | — | .002-.004 | .003-.005 | .004-.006 |
| 56-200 | — | .003-.005 | .004-.006 | .005-.007 |
| 61-200 | — | .007-.009 | — | .009-.011 |
| 63-200 | — | .003-.005 | — | .005-.007 |
| 64-000 / 65-000 | .001-.003 | .002-.004 | .003-.005 | .004-.006 |
| 77-100 (tapered ball) | — | .003-.005 | — | .005-.007 |

Units: inches per tooth. The chart heading is "Recommended Chip Load per
Tooth by Cutting Diameter (in)".

#### Soft Wood (https://onsrud.com/images/Soft%20Wood.pdf, page 114)

| Series | 1/16 | 1/8 | 3/16 | 1/4 |
|---|---|---|---|---|
| 10-00 | .004-.006 | .005-.007 | — | .007-.009 |
| 40-000 | — | .002-.004 | .003-.005 | .004-.006 |
| 40-100 | — | .005-.007 | .005-.007 | .006-.008 |
| 52-200 / 57-200 | — | .006-.008 | .006-.008 | .007-.009 |
| 56-200 | — | .004-.006 | .005-.007 | .006-.008 |
| 61-000 | — | .008-.010 | .009-.011 | .010-.012 |
| 61-200 | — | .008-.010 | — | .010-.012 |
| 63-200 | — | .003-.005 | — | .005-.007 |
| 64-000 / 65-000 | .001-.003 | .002-.004 | .003-.006 | .004-.006 |
| 77-100 (tapered ball) | — | .003-.005 | — | .005-.007 |

#### MDF (https://onsrud.com/images/MDF.pdf, page 116)

| Series | 1/16 | 1/8 | 3/16 | 1/4 |
|---|---|---|---|---|
| 48-000 | — | — | .004-.006 | .005-.007 |
| 52-200 / 57-200 | — | .005-.007 | .006-.008 | .006-.008 |
| 52-400 / 57-400 | — | — | .004-.006 | .005-.007 |
| 56-200 | — | .003-.005 | .004-.006 | .005-.007 |
| 61-200 | — | .007-.009 | .008-.010 | .009-.011 |
| 63-200 | — | .003-.005 | — | .005-.007 |
| 64-000 / 65-000 | .001-.003 | .002-.004 | .003-.005 | .004-.006 |
| 77-100 (tapered ball) | — | .003-.005 | — | .005-.007 |

#### Hard Plywood (https://onsrud.com/images/Hard%20Plywood.pdf, page 118)

Baltic birch plywood has birch faces and birch core. It falls under "Hard
Plywood" in the Onsrud scheme. Onsrud does not name Baltic birch.

| Series | 1/16 | 1/8 | 3/16 | 1/4 |
|---|---|---|---|---|
| 48-000 | — | — | .004-.006 | .005-.007 |
| 52-200 | — | .005-.007 | .006-.008 | .006-.008 |
| 56-200 | — | .003-.005 | .004-.006 | .005-.007 |
| 57-200 | — | .005-.007 | .006-.008 | .006-.008 |
| 61-200 | — | .005-.007 | — | .007-.009 |
| 63-200 | — | .003-.005 | — | .005-.007 |
| 64-000 / 65-000 | .001-.003 | .002-.004 | .003-.005 | .004-.006 |
| 77-100 (tapered ball) | — | .003-.005 | — | .005-.007 |

#### Soft Plywood (https://onsrud.com/images/Soft%20Plywood.pdf, page 117)

| Series | 1/16 | 1/8 | 3/16 | 1/4 |
|---|---|---|---|---|
| 48-000 | — | — | .005-.007 | .005-.007 |
| 52-200 / 57-200 | — | .005-.007 | .006-.008 | .006-.008 |
| 56-200 | — | .003-.005 | .004-.006 | .005-.007 |
| 61-200 | — | .006-.008 | .007-.009 | .008-.010 |
| 63-200 | — | .003-.005 | — | .005-.007 |
| 64-000 / 65-000 | .001-.003 | .002-.004 | .003-.005 | .004-.006 |

The Soft Plywood table has no 77-100 row.

### 1.2 ShopBot Tools — "Feeds and Speeds Charts", July 21 2016

Source: https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf

ShopBot states the origin of the numbers:

> These charts have been taken from Onsrud's recommendations

ShopBot calculates every feed rate at 18,000 RPM. The chart holds one small
tool: the 1/8 in tapered carbide upcut ball end mill, Onsrud series 77-102,
SB# 13636, 2 flutes, cut depth 1 x D.

| Material | Chip load per leading edge | Feed rate (ips) |
|---|---|---|
| Soft wood | .003-.005 | 1.8-3.0 |
| Hard wood | .003-.005 | 1.8-3.0 |
| MDF | .003-.005 | 1.8-3.0 |
| Soft plywood | n/a (not listed) | n/a |
| Laminated chipboard | n/a (not listed) | n/a |
| Laminated plywood | .003-.005 | 1.8-3.0 |

Derived: 1.8-3.0 inches per second equals 108-180 inches per minute.

ShopBot lists three 1/4 in tools. The 1/4 in upcut carbide end mill, Onsrud
52-910, 2 flutes:

| Material | Chip load |
|---|---|
| Soft wood | .007-.009 |
| Hard wood | .006-.008 |
| MDF | .006-.008 |
| Soft plywood | n/a (not listed) |

The 1/4 in downcut carbide end mill, Onsrud 57-910, 2 flutes:

| Material | Chip load |
|---|---|
| Soft wood | .007-.009 |
| Hard wood | .005-.007 |
| MDF | .006-.008 |

The 1/4 in upcut carbide end mill, Onsrud 65-025, 1 flute:

| Material | Chip load |
|---|---|
| Soft wood | .004-.006 |
| Hard wood | .004-.006 |
| MDF | .004-.006 |
| Soft plywood | .004-.006 |

The chart has no 1/16 in row.

### 1.3 Amana Tool — Solid Carbide Compression Spiral Router Bits

Source:
https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Compression-Spirals-v8.pdf

Conditions: 18,000 RPM, depth of cut 1 x tool diameter.

2 flute:

| Diameter | Wood IPM / chip load | MDF-Laminate IPM / chip load | Plywood IPM / chip load |
|---|---|---|---|
| 1/8" | 40" / .0011" | 80" / .0022" | 40" / .0011" |
| 5/32" | 60" / .0017" | 110" / .0031" | 60" / .0017" |
| 3/16" | 80" / .0022" | 160" / .0044" | 80" / .0022" |
| 1/4" | 110" / .0031" | 220" / .0061" | 110" / .0031" |
| 3/8" | 200" / .0056" | 400" / .0111" | 200" / .0056" |
| 1/2" | 280" / .0077" | 400" / .0111" | 280" / .0077" |

1 flute:

| Diameter | Wood IPM / chip load | MDF-Laminate IPM / chip load | Plywood IPM / chip load |
|---|---|---|---|
| 1/8" | 60" / .0031" | 110" / .0062" | 60" / .0031" |
| 1/4" | 60" / .0031" | 110" / .0062" | 60" / .0031" |
| 1/2" | 140" / .0077" | 280" / .0153" | 170" / .0092" |

The same chart gives a "Ramp Down" column and the rule
"To find Ramp Down: Feed Rate IPM / # of flutes".

**Caution: the Amana compression chip loads are three to five times lower
than the Onsrud chip loads at the same diameter and material.** The two
publishers do not agree. The Amana chart fixes the feed rate first. The
Onsrud chart fixes the chip load first.

### 1.4 Freud — "Get the most out of your Freud bits"

Source:
https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf

Condition line: "Recommended* Chip Loads, based on cut depth equal to bit
diameter".

Freud solid carbide router bits:

| Tool dia. | MDF/Particle | Hardwood | Softwood | Plywood |
|---|---|---|---|---|
| 1/8" | .004"-.007" | .002"-.005" | .004"-.006" | .003"-.005" |
| 1/4" | .013"-.017" | .008"-.011" | .010"-.012" | .006"-.009" |
| 3/8" | .018"-.021" | .014"-.016" | .016"-.019" | .015"-.018" |
| 1/2" | .023"-.027" | .018"-.021" | .020"-.023" | .018"-.021" |

Note: the published plywood 1/4 in cell prints as `.006".009"`. The dash is
missing in the source. The agent reads it as .006"-.009" but does not
guarantee it.

Freud carbide tipped straight and profile bits:

| Tool dia. | MDF/Particle | Hardwood | Softwood | Plywood |
|---|---|---|---|---|
| 1/8" | .002"-.004" | .002"-.004" | .003"-.005" | 003"-005" (dash missing) |
| 1/4" | .004"-.006" | .005"-.007" | .006"-.008" | .005"-.006" |
| 1/2" | .006"-.007" | .008"-.010" | .008"-.012" | .007"-.009" |

Freud repeats the depth rule:

> If Cut Depth is 2X the bit diameter, reduce the Chip Load by at least 25%

Freud has no 1/16 in row.

### 1.5 Techno CNC Systems — "Chip Load Chart"

Source:
https://technocnc.com/wp-content/uploads/2023/03/Techno-CNC_Chip-Load-Data_Rev-2-1.pdf

| Tool dia. | Hard Wood | Softwood & Plywood | MDF & Particle Board |
|---|---|---|---|
| 1/8" | .003"-.005" | .004"-.006" | .004"-.007" |
| 1/4" | .008"-.010" | .010"-.013" | .010"-.013" |
| 3/8" | .014"-.018" | .016"-.019" | .014"-.017" |
| 1/2" and up | .019"-.021" | .02"-.023" | .018"-.021" |

Techno states the RPM band: "between 12,000 – 24,000 RPM". Techno repeats
the same 1xD / 2xD / 3xD depth rule. Techno has no 1/16 in row.

### 1.6 PreciseBits — "Calibrating Feeds and Speeds When Using Carbide
Microtools"

Source: https://www.precisebits.com/tutorials/calibrating_feeds_n_speeds.htm

PreciseBits does not publish a diameter table. PreciseBits publishes a rule
that scales with the diameter:

- Softwoods (pine, fir): "F = 0.03 x D x No. flutes x RPM (3% chipload per
  flute)"
- Hardwoods (birch, cherry, maple, rosewood): "F = 0.03 x D x No. flutes x
  RPM (3% chipload per flute)"
- Composites (G10, phenolic, carbon fibre): 0.7% chipload per flute
- Thermoplastics: 8% chipload per flute

Derived from the 3% rule, at one flute:

| Diameter | Chip load per flute |
|---|---|
| 1/16" (1.588 mm) | .0019" (0.048 mm) |
| 1/8" (3.175 mm) | .00375" (0.095 mm) |
| 1/4" (6.35 mm) | .0075" (0.191 mm) |

The 3% rule agrees well with the Onsrud 63-200 and 77-100 rows at 1/8 in
(.003-.005). The 3% rule agrees with the Onsrud 52-200/57-200 hardwood row
at 1/4 in (.005-.007) only at the top of the band.

### 1.7 Small-diameter caveat that ShopBot and Onsrud both state

ShopBot repeats the Onsrud optimisation method:

> Start off using an RPM derived for the chip load for the material being cut

ShopBot then tells the operator to raise the feed until the finish falls
off, and then to cut the feed by 10%. Both publishers treat the chart as a
starting point, not as a specification.

---

## 2. Ball nose and tapered ball nose finishing in wood

### 2.1 Stepover as a fraction of diameter

| Publisher | Type | Finishing stepover | Source |
|---|---|---|---|
| PreciseBits | Vendor | 0.08 x tip diameter (8%); roughing clearance 0.40 x tip diameter | https://www.precisebits.com/products/carbidebits/taperedcarve250b2f.asp |
| Vectric (VCarve Pro V12) | CAM vendor | "8 - 12% of the tool diameter is typical" | https://docs.vectric.com/docs/V12.0/VCarvePro/ENU/Help/form/Finish%20Machining%20Toolpath/index.html |
| IDC Woodcraft | Retailer | Ball nose 8%; taper ball nose 5-8% | https://community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf |
| DAPRA | Vendor (metal inserts) | "Stepover should be greater than or equal to DOC" | https://www.dapra.com/ball-nose/application-information |

Vectric also writes:

> Variations on this tool type such as a tapered Ball Nosed cutter will also
> work

PreciseBits publishes the full tool table for the 2-flute tapered ball nose:

| Tip diameter | Taper angle | Tip radius | Max depth |
|---|---|---|---|
| 0.0625" | 3.6° | 0.0313" | 1.50" |
| 0.1250" | 2.5° | 0.0625" | 1.50" |
| 0.2498" | 0.001° | 0.1249" | 1.50" |

PreciseBits gives the depth per pass as 1 x tip diameter, and the maximum
as 2 x tip diameter. PreciseBits gives the spindle band as "5 KRPM to 60
KRPM". PreciseBits refuses to publish a feed rate. PreciseBits sends the
operator to its "Sweetspot Test" instead.

### 2.2 Depth of cut for a finishing pass

DAPRA publishes one figure:

> Maximum Depth of Cut (DOC) for finishing should be less than or equal to
> 10% of ball diameter

Source: https://www.dapra.com/ball-nose/application-information

DAPRA sells indexable ball nose tooling for metal. DAPRA does not publish
wood data.

### 2.3 Chip load rows for small ball nose tools in wood

Amana Tool, "Spektra Extreme Tool Life Coated 2D/3D Carving CNC Solid
Carbide Router Bits", 18,000 RPM:

Source:
https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf

| Tool | Diameter | Material | IPM | Chip load per tooth |
|---|---|---|---|---|
| 2 flute ball nose | 1/4" (0.250") | Wood, MDF, Sign-Foam | 250"-320" | 0.007"-0.009" |
| 3 flute ball nose | 1/32" (0.031") | Wood, MDF, Sign-Foam | 40"-108" | 0.00075"-0.002" |
| 3 flute ball nose | 1/8" (0.125") | Wood, MDF, Sign-Foam | 80"-100" | 0.0015"-0.0025" |
| 4 flute ball nose & flat bottom | 1/16" (0.0625") | Wood, MDF, Sign-Foam | 35"-45" | 0.0005"-0.00065" |
| 4 flute ball nose & flat bottom | 1/8" (0.125") | Wood, MDF, Sign-Foam | 35"-45" | 0.0005"-0.00065" |
| 3 flute extra long ball nose | 1/4" (0.250") | Wood, MDF, Sign-Foam | 215"-320" | 0.004"-0.006" |

**Caution: this Amana chart states the same depth condition as the others:
"Depth of Cut: 1 x D Use recommended chip load". It does not state a
stepover. It is not a light-finishing-pass chart.** The 4-flute 1/16 in and
1/8 in rows carry the same feed band, 35"-45" IPM, for two different
diameters. That is a published fact, not an error the agent can correct.

IDC Woodcraft publishes both the stepover and the feed together:

| Bit | Cut dia. | Flutes | Feed (in/min) | Depth per pass | Stepover | Spindle |
|---|---|---|---|---|---|---|
| 1/8" ball nose | 0.125 | 2 | 60 | 0.05 | 8% | 22,000 |
| 1/4" ball nose | 0.25 | 2 | 70 | 0.12 | 8% | 19,000 |
| Taper ball nose, 0.015 tip radius | 0.250 | 2 | 60 | 0.25 | 5-8% | 19,000 |
| Taper ball nose, 0.045 tip radius | 0.250 | 2 | 60 | 0.25 | 5-8% | 19,000 |

IDC writes: "Stepover is set for 3D modeling work."

Derived from the IDC ball nose rows: 60 / (22,000 x 2) = 0.00136 inch per
tooth at 1/8 in; 70 / (19,000 x 2) = 0.00184 inch per tooth at 1/4 in.

**These derived IDC chip loads are about one quarter of the Amana 2-flute
ball nose value at 1/4 in (0.007"-0.009").** The two publishers do not
agree. IDC writes for benchtop routers. Amana writes for industrial
spindles.

### 2.4 Does any publisher say to raise the feed for chip thinning?

Yes, but the agent found this only from metalworking publishers.

DAPRA publishes a feed rate adjustment factor and states the formula:

> (RPM x FPT x FRA = IPM)

FRA is the "Feed Rate Adjustment factor". DAPRA publishes a table of FRA
against depth of cut. Source:
https://www.dapra.com/ball-nose/feed-speed-dia-compensation

| Depth of cut | FRA, 1/4" ball | FRA, 1/2" ball |
|---|---|---|
| .005 | 3.6 | 5.0 |
| .010 | 2.6 | 3.6 |
| .015 | 2.1 | 2.9 |
| .020 | 1.8 | 2.6 |
| .025 | 1.7 | 2.3 |
| .050 | 1.2 | 1.7 |
| .075 | 1.1 | 1.4 |
| .100 | — | 1.2 |
| .125 | — | 1.2 |
| .150 | — | 1.1 |

Harvey Performance states the rule in prose:

> Once the RDOC falls below 50% of the cutter diameter […] the maximum chip
> thickness decreases

Source: https://www.harveyperformance.com/in-the-loupe/combat-chip-thinning/

**The agent found no tooling manufacturer that publishes a chip-thinning
feed increase for wood.** Every chip-thinning source the agent found writes
about metal.

---

## 3. The published formulas

### 3.1 Radial chip thinning below half diameter

DAPRA publishes the effect as a table of multipliers, not as an equation.

Source: https://www.dapra.com/articles/radial-chip-thinning

| Width of cut (WOC) | Adjusted feed per tooth multiplier |
|---|---|
| 100% | 1.00 (chip thickness equals programmed FPT) |
| 50% | 1.00 (still at maximum) |
| 35% | 1.05 |
| 10% | 1.70 |
| 5% | about 2.3 ("130% productivity increase") |

DAPRA writes:

> At 35% WOC, we start to see things change a bit

Derived check: the standard radial chip thinning factor is
`RCTF = 1 / sqrt(1 - (1 - 2*ae/D)^2)`. At ae/D = 0.35 it gives 1.048. At
0.10 it gives 1.667. At 0.05 it gives 2.294. Those three values match the
DAPRA table to the published precision. The agent states this as a
consistency check, not as a DAPRA quotation.

Harvey Performance publishes the equation as an image file only. The agent
could not transcribe it reliably. One extraction attempt returned
`IPT = (CT x RDOC) / (D / 2)`. That form decreases IPT as RDOC falls, which
contradicts the prose on the same page. **The agent therefore reports the
Harvey equation as unverified and does not use it.**

### 3.2 Ball nose effective cutting diameter at a given axial depth

DAPRA publishes the RPM rule in text:

> (SFM x 3.82 / ECD = RPM)

DAPRA publishes ECD as a table. Source:
https://www.dapra.com/ball-nose/feed-speed-dia-compensation

| Depth of cut | ECD, 0.250" ball | ECD, 0.500" ball |
|---|---|---|
| .005 | .070 | .099 |
| .010 | .098 | .140 |
| .015 | .119 | .171 |
| .025 | .150 | .218 |
| .035 | .173 | .255 |
| .050 | .200 | .300 |
| .100 | .245 | .400 |
| .125 | .250 | .433 |
| .150 | — | .458 |
| .200 | — | .490 |
| .250 | — | .500 |

Derived check: the geometric identity is
`ECD = 2 * sqrt(D*ap - ap^2)`. At D = 0.500 and ap = 0.010 it gives 0.1400.
At D = 1.000 and ap = 0.100 it gives 0.6000. Both match the DAPRA table.
The 0.250 in column matches to within one unit in the third decimal place.

Tyson Tool publishes a formula sheet with the heading "To calculate
effective diameter of ball nose tool" and the legend `de = Effective
diameter`, `d = Diameter of milling cutter, in inches`, `ap = Depth of cut`.
**The equation itself is a placed image. The agent could not extract it.**
Source: https://tysontool.com/tech-mill-formulas.pdf

Harvey Performance also publishes this formula as an image only. Source:
https://www.harveyperformance.com/in-the-loupe/ball-nose-milling-strategy-guide/

### 3.3 Cusp height from stepover for a ball nose

CutViewer publishes the equation in text:

> Cusp Height = R − √(R² − (S/2)²)

with "R = ball radius, S = stepover distance". Source:
https://cutviewer.com/tools/stepover-calculator/

CutViewer publishes no inverse. The page says CAM software does the
inversion.

Tyson Tool lists "To calculate scallop height (cusp height)" with the legend
`h = Scallop height` and `s = Stepover value between two cutting passes, in
inches`. **The equation is an image. The agent could not extract it.**
Source: https://tysontool.com/tech-mill-formulas.pdf

**CutViewer is a CAM simulation publisher, not a tooling manufacturer. The
agent found no tooling manufacturer that publishes the cusp height equation
as text.**

---

## 4. Minimum chip thickness and the rubbing threshold

### 4.1 What the wood tooling vendors say

No wood tooling vendor publishes a numeric minimum chip thickness. They
describe the failure instead.

Freud writes:

> If your chip is very small, or just sawdust, then it will not carry enough
> heat away

Source:
https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf

PreciseBits writes:

> If your feed to too low for the spindle speed, the chipload is too small.

PreciseBits then describes fine powder that packs the kerf, and describes
the edge rounding over from abrasion. PreciseBits publishes one numeric
floor, and it is for brittle materials, not for wood:

> start with an initial feedrate that yields a TOTAL chip load less than 2%

Source: https://www.precisebits.com/tutorials/calibrating_feeds_n_speeds.htm

ShopBot writes:

> When chip load is too small, bits will get too hot and dull quicker.

Source: https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf

### 4.2 The one numeric rubbing threshold the agent found

CNCCookbook reports a vendor figure from Ingersoll, for carbide in metal:

> carbide chip loads should not be less than 0.004″ or you run the risk of
> rubbing

Source: https://www.cnccookbook.com/chip-thinning-rubbing-lesson-3-fs-email/

**Caution: 0.004 in exceeds the whole published wood chipload band for a
1/8 in router bit. Every wood chart above puts a 1/8 in tool between .002
and .007 in per tooth. The Ingersoll figure is a metalworking figure. Do
not carry it into wood.**

### 4.3 The edge-radius rule from the literature

Wojciechowski, S., "Estimation of Minimum Uncut Chip Thickness during
Precision and Micro-Machining Processes of Various Materials—A Critical
Review", *Materials* (Basel), 2021.
Source: https://pmc.ncbi.nlm.nih.gov/articles/PMC8745993/

The review defines `k = h_min / r_n`, where `h_min` is the minimum uncut
chip thickness and `r_n` is the cutting edge radius. The review reports the
range across materials and processes as:

> 0.08 ≤ k ≤ 0.63

Below `h_min` the review describes elastic-plastic deformation and "intense
ploughing of the material". No chip forms.

Other reported ratios:

| Ratio | Context | Source |
|---|---|---|
| 1/4 to 1/3 of the edge radius | General practical range | https://www.sciencedirect.com/topics/engineering/minimum-chip-thickness |
| 0.25 | OFHC copper, 10° rake, 2 µm edge radius | https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7600950 |
| 0.25 to 0.30 | 304 stainless steel | https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12898621/ |
| 5% to 20% | quoted range | https://www.cnccookbook.com/chip-thinning-rubbing-lesson-3-fs-email/ |

Edge radius figures the agent found:

| Value | Context | Source |
|---|---|---|
| 1 to 5 µm | described as a "sharp" cutting edge | https://www.sciencedirect.com/topics/engineering/cutting-edge-radius |
| 2 to 5 µm | fine grained tungsten carbide, TiAlN coated, 3 flute | https://www.researchgate.net/publication/265168951 |
| 1.5 µm new, 25 µm after a 600 mm path | micro-milling AISI 4340 | https://pmc.ncbi.nlm.nih.gov/articles/PMC8745993/ |

Derived, for orientation only: a 3 µm edge radius with k = 0.25 gives
h_min = 0.75 µm, which is 0.00003 in. That is two orders of magnitude below
every published wood chipload. **The edge-radius rule therefore does not
explain wood burnishing on its own.**

**No source the agent found applies the minimum-chip-thickness model to
wood.** Every measurement above comes from metal.

---

## 5. V-bit and engraving bit feeds in wood

### 5.1 Amana Tool — solid carbide 30°, 45° and 60° single flute engraving

Source:
https://www.amanatool.com/pub/media/productattachments/30-45-60-Degree-Engraving-Speed-Chart-v8.pdf

Conditions: 18,000 RPM, depth of cut 1 x tool diameter. The column heading
is "Chip Load Per Tooth IPR".

| Material | 30° (tip 0.005"-0.090") | 45° (tip 0.025"-0.042") | 60° (tip 0.005"-0.090") |
|---|---|---|---|
| Soft Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 70"-100" IPM, 0.004"-0.006" |
| Hard Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 70"-100" IPM, 0.004"-0.006" |

Tool reference numbers for the 30° column include 45771, 45772, 45773,
45774, 45775, 45776, 45777 and 45779. All take a 1/4 in shank.

The depth rule on this chart differs from the endmill charts. It reduces the
**feed rate**, not the chip load:

> Depth of Cut: 1 x D Use recommended feed rate / 2 x D Reduce feed rate by
> 25% / 3 x D Reduce feed rate by 50%

### 5.2 Amana Tool — 2 flute V-groove engraving 15°, 60° and 90°

Source:
https://www.amanatool.com/pub/media/productattachments/15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf

| Material | 15° | 60° | 90° |
|---|---|---|---|
| Soft Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" |
| Hard Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" |

### 5.3 Amana Tool — Spektra coated 15°, 30°, 45° and 120° single flute

Source:
https://www.amanatool.com/pub/media/productattachments/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf

| Material | 15° (tip 0.005") | 30° (tip 0.005"-0.030") | 45° (tip 0.042") | 120° (tip 0.015") |
|---|---|---|---|---|
| Soft Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 40"-110" IPM, 0.002"-0.006" |
| Hard Wood | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 50"-125" IPM, 0.003"-0.007" | 40"-110" IPM, 0.002"-0.006" |

### 5.4 IDC Woodcraft — V-bit table (retailer, benchtop routers)

Source:
https://community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf

| Bit | Cut dia. | Flutes | Feed (in/min) | Plunge | Depth per pass | Clear pass stepover | Final pass stepover | Spindle |
|---|---|---|---|---|---|---|---|---|
| 30° V-bit | 0.25 | 1 | 15 | 35 | 0.025 | 20% | 0.005 | 27,000 |
| 60° V-bit | 0.25 | 2 | 30 | 60 | 0.05 | 20% | 0.005 | 22,000 |
| 90° V-bit | 0.25 | 2 | 45 | 45 | 0.1 | 20% | 0.005 | 17,000 |
| 120° V-bit | 1.0 | 2 | 60 | 80 | 0.19 | 30% | 0.02 | 17,000 |
| 150° V-bit | 1.5 | 2 | 75 | 60 | 0.150 | 30% | 0.02 | 17,000 |

Derived: the 30° row gives 15 / (27,000 x 1) = 0.00056 inch per tooth. The
60° row gives 30 / (22,000 x 2) = 0.00068 inch per tooth. The 90° row gives
45 / (17,000 x 2) = 0.00132 inch per tooth.

**Caution: the IDC derived chip loads sit four to ten times below the Amana
published band of 0.003"-0.007". The Amana feed band of 50-125 IPM sits two
to eight times above the IDC feed. The two publishers disagree by a large
factor on the same operation.**

### 5.5 Depth about 1 mm

The requested case is a cut about 1 mm (0.039 in) deep. The IDC 30° row
allows 0.025 in (0.635 mm) per pass. The IDC 60° row allows 0.05 in
(1.27 mm) per pass. A 1 mm cut therefore needs two passes with the 30° bit
and one pass with the 60° bit, by the IDC table.

The Amana charts state "Depth of Cut: 1 x Tool Diameter". **For a V-bit that
condition is ambiguous. A V-bit has no single cutting diameter. The agent
cannot resolve which diameter Amana means. Treat the Amana depth rule as
unusable for a V-bit without a clarification from Amana.**

---

## 6. Gaps — what is NOT published

The agent lists here every figure it could not source. The agent invented
nothing to fill these gaps.

1. **A 20 degree V-bit chart.** Amana publishes 15°, 30°, 45°, 60°, 90° and
   120°. Amana publishes no 20° chart. The agent found no 20° chart from any
   vendor. Bits & Bits publishes no feeds page the agent could reach.
   Widgetworks publishes no chart the agent could reach.

2. **Baltic birch by name.** No vendor names Baltic birch. Onsrud splits
   plywood into "Hard Plywood" and "Soft Plywood" only. The agent places
   Baltic birch under Hard Plywood on species grounds, not on a published
   statement.

3. **A 1/16 in (1.6 mm) row in most wood charts.** Only these publishers go
   that small:
   - Onsrud, series 64-000/65-000 only: .001-.003 in, in all four wood
     tables.
   - Onsrud, series 10-00, soft wood only: .004-.006 in.
   - Amana Spektra 4-flute tapered ball nose: 0.0005"-0.00065" in wood.
   Freud, Techno CNC, ShopBot and the Amana compression chart all stop at
   1/8 in.

4. **Harvey Tool wood data.** Harvey Tool publishes chip thinning and ball
   nose theory. Harvey Tool publishes no chipload for wood. The agent found
   no Harvey small-diameter wood guidance at all.

5. **The radial chip thinning equation as vendor text.** Harvey publishes it
   as an image. Machining Doctor returned HTTP 403 on every attempt. DAPRA
   publishes multipliers, not an equation. The agent has no verbatim vendor
   equation.

6. **The ball nose effective diameter equation as vendor text.** Harvey and
   Tyson Tool both publish it as an image. DAPRA publishes a table only.

7. **The cusp height equation from a tooling manufacturer.** Tyson publishes
   it as an image. The only text version the agent found comes from
   CutViewer, a CAM simulation publisher.

8. **A minimum chip thickness for wood.** No publisher gives one. The
   0.004 in Ingersoll figure is for carbide in metal. The k = h_min / r_n
   literature is entirely metal and micro-machining.

9. **A published carbide edge radius for a wood router bit.** Every edge
   radius figure the agent found comes from metal-cutting research. No
   router bit maker publishes an edge radius.

10. **A chip-thinning feed increase recommended for wood.** DAPRA, Harvey,
    Helical and Datron all write about metal. No wood tooling vendor tells
    the operator to raise the feed at a light stepover.

11. **Vortex Tool's chart.** The published link
    `http://www.vortextool.com/images/chipLoadChart.pdf` returns HTTP 404.
    Third-party articles quote two Vortex cells (1/2 in MDF .025-.027 in,
    1/2 in plywood .021-.023 in). The agent did not reach the chart itself
    and does not reproduce second-hand cells as Vortex data.

12. **Onsrud's own `xdoc/FeedSpeeds` page.** ShopBot cites
    `http://www.onsrud.com/xdoc/FeedSpeeds`. That URL now returns HTTP 404.
    The per-material PDFs listed in section 1.1 replace it.

13. **A wood-specific stepover from a tooling manufacturer for a plain ball
    nose.** PreciseBits publishes 8% for its tapered ball nose. The agent
    found no equivalent figure from Amana, Onsrud, Freud or Vortex for a
    plain ball nose in wood. The 8-12% figure comes from Vectric, a CAM
    vendor.

---

## 7. Cross-publisher disagreements to carry forward

The agent records these as facts about the published data, not as errors.

| Case | Low publisher | High publisher | Ratio |
|---|---|---|---|
| 1/4" 2-flute, wood | Amana compression, .0031" | Onsrud 52-200 soft wood, .007-.009" | about 2.3 to 2.9 |
| 1/4" 2-flute ball nose, wood | IDC derived, .00184" | Amana Spektra, .007-.009" | about 3.8 to 4.9 |
| 1/4" V-bit, hard wood | IDC derived, .00056-.00132" | Amana, .003-.007" | about 2.3 to 12.5 |
| 1/8" hardwood | Freud, .002-.005" | Techno CNC, .003-.005" | overlapping |
| 1/8" hardwood | Onsrud 52-200, .003-.005" | Onsrud 40-000, .006-.008" | 2.0 within one publisher |

The last row matters most. Onsrud's own numbers vary by a factor of two at
the same diameter and the same material, because the tool geometry differs.
**A single chipload number per diameter and material does not describe the
published data. The tool series is a first-class input.**
