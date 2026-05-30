# Citeable Primary-Source Acquisition List for a CAM Feeds/Speeds & Tool-Load Dataset

## TL;DR
- **The richest, highest-grade feeds/speeds primary sources are Amana Tool's per-series chip-load PDFs and LMT Onsrud's cutting-data sheets** — together they directly populate chipload, RPM, and depth-of-cut fields for nearly every tool family across wood, sheet goods, plastics, and aluminum, with Onsrud being the single best source for closing the plastics gap (it tabulates HDPE, polycarbonate, acrylic, and nylon/acetal-class plastics explicitly with O-flute/upcut/downcut/compression geometry).
- **Small-diameter (<3 mm) and aluminum gaps are best closed by Harvey Tool, Helical Solutions, and Garr Tool**, whose downloadable speeds-and-feeds charts and the shared Machining Advisor Pro engine cover micro tooling (Harvey's metric ball end mills go "down to just .05mm") and aluminum-specific SFM/chipload by diameter; for Kc, Sandvik Coromant and Kennametal publish the kc1.1 + mc Kienzle coefficients — but for metals only.
- **For material properties, use FPL Wood Handbook + Wood Database (wood Kc/Janka), MatWeb/ASM + supplier datasheets (Shore D and Brinell), and note an important limitation: no published source gives a Sandvik-style kc1.1/mc pair for plastics** — plastics Kc must be back-calculated from measured-force research papers or estimated from strength/hardness datasheets.

## Key Findings

This report is organized by data type: (A) LUT feeds/speeds sources, (B) specific cutting force Kc sources, and (C) hardness sources. Each source lists the data fields it populates, evidence grade, the citeable URL with page/PDF notes, and access caveats. Gap-closing relevance (plastics breadth, small-diameter <3 mm, aluminum) is flagged throughout.

Evidence-grade convention used here:
- **Grade A**: direct manufacturer chart tying a specific tool/series and material to chipload/RPM/ap.
- **Grade B**: derived values — research-measured Kc, or strength/hardness used to estimate Kc.
- **Grade C**: community/aggregator charts useful only for cross-checking derived rows.

---

## A. LUT Feeds/Speeds Sources (chipload, RPM, ap/ae, flute count)

### A1. Amana Tool — per-series chip-load PDFs (Grade A; highest priority)
Amana publishes one PDF per bit series, each tabulating, per material column (typically Softwood, Hardwood, Plywood/Chipboard, MDF/Laminate, Plastic, and often Aluminum), the feed rate (IPM at 18,000 RPM), chip load per tooth (inch), and depth-of-cut rule. The standard DOC rule on every chart is: "1×D use recommended chip load; 2×D reduce by 25%; 3×D reduce by 50%," which directly supplies ap guidance. All charts include the conversion formulas (RPM = SFM×3.82/D; Feed = RPM×flutes×chipload). Host domain: amanatool.com/pub/media/productattachments/. These are Grade A and free (no login).

Specific located PDFs and what they cover:
- **Spiral Ball Nose** (`Spiral-Ball-Nose-Speed-Chart-v6.pdf`): 2-flute ball nose, up-cut and down-cut, diameters 1/16"–3/4"; columns include Aluminum, Softwood, Hardwood, etc. Closes ball-nose + some aluminum. (Verified: Aluminum 1/16" column = 70"–150" IPM at 0.002"–0.004" chipload, rising to 430"–580" IPM at 0.014"–0.016" for 3/4".)
- **Compression Spirals** (`Solid-Carbide-Compression-Spirals-v8.pdf`): compression geometry, diameters 1/8"–1/2", columns Wood/MDF-Laminate/Plywood/Plastic with feed, chipload, and ramp-down. Good for plywood/MDF.
- **Plastic O-Flute** (`Plastic-O-Flute-Speed-Chart-v2.pdf` and Spektra variant `Spektra-Plastic-O-Flute-Speed-Chart-v11.pdf`): O-flute geometry for plastics. Helps plastics breadth.
- **ZrN Aluminum O-Flute** (`ZrN-Aluminum-O-Flute-Speed-Chart-v13.pdf`): aluminum-specific O-flute, ZrN-coated. Closes aluminum.
- **V-Groove/Engraving**: `15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf`, `AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf`, `Insert-V-Groove-Speed-Chart-v8.pdf`, `Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf`. A single-flute engraving chart (mirrored at toolstoday.com `Feed-Speed-for-Amana-Tool-30-45-60-Degree-Engraving.pdf`) tabulates 30°/45°/60° with feed/chipload per material (Soft Wood, Hard Wood, Soft Plastic, Hard Plastic, Aluminum, Solid Surface) — e.g., 30°/45° give 50"–125" IPM at 0.003"–0.007" chipload across all those materials. Closes V-bit/chamfer family.
- **Spoilboard/Surfacing** (`Spoilboard_2_2-Speed-Chart.pdf`): 2+2 surfacing/spoilboard bits (RC-2251, RC-2252) with RPM/IPM/chipload per material. Closes facing/surfacing family.
- **Spektra 3D Profiling / Carving** (`Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf`, `ZrN-3D-Profiling-Feed-Chip-Load-Chart-v8.pdf`): tapered ball/3D carving bits; some columns down to 1/32".
- **Carbon/composite** (`File-1436543087.pdf`): for CFRP/fiberglass (out of core scope but available).

Caveat: charts are at fixed 18,000 RPM; RPM range must be derived via the chip-load formula. PDFs are page-1/2 layouts; cite by file name + page. Mirror copies exist at toolstoday.com and woodworkerexpress.com.

### A2. LMT Onsrud — Cutting Data Recommendations (Grade A; best for plastics)
Onsrud's "Cutting Data Recommendations" hub (onsrud.com/Forms/Cutting-Data-Recommendations.asp) links per-material data sheets that give chip load per tooth ranges by tool series and cutting-edge diameter, plus RPM notes and the chipload formula. The depth-of-cut convention is stated verbatim on the data sheets: *"Onsrud bits are typically allowed a cut depth per pass equal to the cutting edge diameter unless otherwise specified… For twice the depth of cut, reduce the chip load per tooth by 25% and for triple the depth of cut, reduce the chip load by 50%"* — this directly supplies ap rules. Material-specific sheets located:
- **Soft Plastic** (`onsrud.com/images/Soft Plastic.pdf` and `S Plastic Cutting Data2.pdf`): ABS, polycarbonate, polyethylene/HDPE, PVC, polypropylene, polystyrene, extruded acrylic, UHMW. This is the single best plastics-breadth source. Text-extractable.
- **Hard Plastic** (Scribd "2012 LMT Onsrud Production Cutting Tools Hard Plastic"): cast acrylic, melamine, nylon, PVC, vinyl; chip load per tooth by CED, with separate single-pass vs roughing/finishing tool lists for <1/2" and >1/2" diameters.
- **Hardwood** (`H Wood Cutting Data2.pdf`) and **Composite** (`Composite Cutting Data2.pdf`).
- **Aluminum**: an aluminum cutting-data sheet is referenced in the CEC/cuttingedgecarbide mirror of the Onsrud data set.
- **Product catalog** (`LMT Onsrud Product Cutting Tools Catalog PCT-19.pdf`) and a master cutter listing (mirrored at microfence.com/wp-content/uploads/2017/04/Onsrud.pdf) give O-flute single/double-edge series (10-00, 11-00 series) with CED/CEL/SHK/OAL dimensions for HDPE/UHMW/acrylic/polycarbonate.
- **Application articles**: "Routing Polycarbonate Material" (onsrud.com/articles/Routing-Polycarbonate-Material.asp) states verbatim that *"the optimum chipload to achieve the best finish seems to be in the range of 0.004 to 0.012. In the case of polycarbonate or a soft plastic, this provides the best finish by properly curling the chips during the routing process,"* plus conventional-vs-climb guidance (soft plastics — HDPE, UHMW, PP — favor conventional cutting; harder acrylic/PC/nylon can favor climb at <3/8").

Flags: closes **plastics breadth** (HDPE, PC, nylon/acetal-class, acrylic) decisively. Caveat: several Onsrud `/images/*.pdf` data sheets are image-only (not text-extractable) and must be read visually/OCR'd; the wood data sheet returned no machine-readable text.

### A3. Whiteside Machine Company — category/product pages (Grade A–B; V-groove + plywood)
Whiteside product pages (whitesiderouterbits.com/products/...) give recommended RPM ranges per bit but generally NOT chipload tables. Examples: 60° V-Groove bits #1540/#1550 list "Materials: natural woods, composite woods, hard plastics; Recommended RPM 13,000–15,000 (max 18,000)"; roughing up/down spirals (RU/RD series) list 16,000–18,000 (max 24,000) for plywood/melamine/MDF. Useful for RPM bounds and material applicability on V-groove and plywood-optimized spirals, but feed/chipload must be derived. Whiteside does not publish a master chipload chart; the community references Freud's published CNC chip-load chart (freudtools.com router-bit-feed-and-speed PDF) as the closest analog. Grade B when deriving feeds.

### A4. Harvey Tool — speeds & feeds charts + Machining Advisor Pro (Grade A; closes small-diameter)
Harvey Tool (harveytool.com/resources/speeds-feeds and /speeds-feeds-guide) publishes downloadable per-product-line speeds & feeds charts and the free Machining Advisor Pro (MAP) calculator (harveyperformance.com/machining-advisor-pro). Coverage includes miniature Square, Ball, Corner Radius, and Tapered end mills in many diameters/reaches, across material groups including Aluminum Alloys, Plastics, and Composites. Critically for the **small-diameter <3 mm gap**: Harvey's "Miniature End Mills – Ball – Stub & Standard – Metric" line is stocked "in metric dimensions and tolerances with cutter diameters down to just .05mm," and tapered ball mills start at 0.015" cutter diameter (stocked in 0.5°–15° tapers). Tapered-ball product pages each link a downloadable standard speeds & feeds set. The "Ball Nose Milling Strategy Guide" (harveyperformance.com/in-the-loupe) gives effective-cutting-diameter and ADOC tables for ball-nose compensation. Caveat: per-tool charts are downloadable PDFs/spreadsheets tied to tool numbers; MAP requires entering a tool/material/setup (no bulk export). Closes **small-diameter** and adds aluminum + plastics.

### A5. Helical Solutions — speeds & feeds charts + Machining Guidebook (Grade A; aluminum + tapered)
Helical (helicaltool.com/resources/speeds-feeds) supplies comprehensive speeds & feeds charts "for every product in its catalog," sharing the MAP engine with Harvey. Strong on **aluminum** (dedicated aluminum end-mill families, high-feed aluminum, chipbreaker roughers) and **tapered end mills**: HTPR-4 (4-flute ball) and HTPR-5 (5-flute square) in 0.5°/1°/1.5°/2°/3°/5° per-side angles and 1/8", 3/16", 1/4" diameters. The **Helical Machining Guidebook (2016)** — a citeable PDF mirrored at web.mae.ufl.edu/designlab/Advanced Manufacturing/Helical_Machining_Guidebook.pdf — contains the chip-thinning calculation for RDOC <50% of diameter (p.22), ramping/HEM guidance (pp.24–25), and the speeds & feeds and common-milling-calculation sections (pp.53–54). Closes **aluminum** and tapered-ball geometry. Caveat: charts tied to specific tool series; material list includes Aluminum, Plastic, Wood, Composites.

### A6. Garr Tool — milling guides for aluminum + technical PDF (Grade A; aluminum + micro)
Garr Tool publishes citeable technical PDFs at garrtool.com:
- `doc/pdf/tech/TECH_MILLING_ALUMINUM.pdf`: aluminum milling guide giving, per series (242M/842M/A3 low-range; 142M/143M/A3 mid-range), SFM ranges (e.g., 400–600 slotting / 1500–2000 profiling) and chip-per-tooth as **% of diameter** (0.5–2.5% by operation), with explicit axial = 0.5×D or 1×D and radial = 0.5×D engagement — directly populates ap/ae fields.
- `wp-content/uploads/2018/11/TECHNICAL.pdf`: broader speeds/feeds with metric SMM ranges and notes (chip thinning, plunge feed reduction 50%, 20%-of-diameter engagement).

Garr's X3/G3 series targets small precision tools **1/32"–1/4"** (helps small-diameter). The Garr feeds & speeds calculator is powered by the G-Wizard engine. Closes **aluminum** and contributes **small-diameter**.

### A7. Community / cross-check feed sources (Grade C)
- **IDC Woodcraft**: chipload calculator (idcwoodcraft.com/pages/chipload-calculator) and downloadable Millmage tool-library databases for Vectric/Carbide Create/Fusion 360 (idcwoodcraft.com/pages/database-downloads); a community-hosted PDF "CNC Router Bit Feeds & Speeds Provided by IDC Woodcraft" exists on the Carbide3D forum. Grade C cross-check for wood/sheet-goods derived rows.
- **Carbide Processors Router Bit Chip Load Chart** (carbideprocessors.com): generic chipload-by-diameter table; note its caveat that results hold up to ~0.017 chipload. Grade C.
- **CNCCookbook / G-Wizard** (cnccookbook.com): physics-engine calculator (≈60 variables, chip thinning, HSM); not a static table but a Grade C derivation cross-check.
- **Cutter Shop / Cutting Edge Carbide (CEC)**: mirror Onsrud chipload data with calculators; Grade C.

---

## B. Specific Cutting Force (Kc) Sources

### B1. Sandvik Coromant — kc1.1 + mc (Grade A for metals; aluminum)
Sandvik defines kc1.1 verbatim as "the force, Fc, in the cutting direction… needed to cut a chip area of 1 mm that has a thickness of 1 mm," valid for a neutral insert at 0° rake, and gives the model kc = kc1.1·h^(−mc) (their pages use kc1 notation). The Sandvik "Specific cutting force" knowledge page (sandvik.coromant.com/en-us/knowledge/materials/specific-cutting-force) and "Milling formulas and definitions" page provide the formula and the rake-angle correction. The **Metal Cutting Technology Training Handbook** (mirrored as a PDF at pdfcoffee.com) tabulates kc1 ranges by ISO group: P (steel) 1500–3100 N/mm²; M (stainless) 1800–2850; K (cast iron) 790–1350; **N (aluminum/non-ferrous) 350–1350 N/mm²**; S 2400–3100; H 2550–4870. The "Technical Guide – Materials ISO" PDF (mirrored on ResearchGate) and CMC material-classification system give finer sub-groups. Aggregator **Machining Doctor** (machiningdoctor.com/glossary/specific-cutting-force-kc-kc1) reproduces kc1.1 + mc for 41 material groups and a detailed SAE/DIN/Wnr chart (mc 0.2–0.3 typical) — Grade B/C convenient lookup. Closes **aluminum Kc**. Caveat: Sandvik removed the standalone Kc charts from its current website; the handbook PDFs are the citeable primary.

### B2. Kennametal — machining data / force-torque-power (Grade A/B for metals)
Kennametal's engineering calculators (kennametal.com/.../engineering-calculators/z-axis-milling/force-torque-and-power.html) compute tangential force, torque, and power and embed Kc values; the Kennametal technical data (Scribd "Kennametal Technical Data Metric" / "Kennametal Speed and Feed Guidelines") tabulates specific cutting force Kc in MPa vs feed (0.1–0.6 mm/rev) for various work materials, and documents machinability/lead-angle/wear factors. Useful as a second metals-Kc primary to cross-check Sandvik. Caveat: Kennametal's website no longer exposes a single comprehensive Kc list; values are embedded in calculators and catalog PDFs.

### B3. Wood & wood-composite Kc — research literature (Grade B; derived)
- **Pałubicki (2021), "Cutting Forces in Peripheral Up-Milling of Particleboard,"** Materials 14(9):2208, DOI 10.3390/ma14092208 (open access, MDPI/PMC): measured average specific principal cutting force of exactly **32.0 N/mm² (slow) and 37.6 N/mm² (fast milling)** for particleboard, with the 60 m/s speed increasing the average value 17.5% vs 40 m/s, and kc decreasing as instantaneous uncut chip thickness rises (matching MDF magnitudes). Directly citeable for particleboard/MDF Kc.
- **"Specific cutting coefficients for the most common engineered wood products"** (HAL hal-04274766): reports specific cutting coefficients and intercepts for particleboard, MDF, OSB, and plywood; finds particleboard and MDF nearly isotropic, plywood moderately directional. Best multi-material EWP Kc source.
- **"Specific Cutting Forces of Isotropic and Orthotropic Engineered Wood Products by Round Shape Machining"** (PMC6315737): method + values for PTFE, MDF, beech/poplar LVL, including orthotropic anisotropy treatment.
- **USDA FPL Wood Handbook (GTR-190, 2010), Chapter 5 "Mechanical Properties of Wood"** (citeable PDF at precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf; full handbook at research.fs.usda.gov/treesearch/download/37440.pdf): shear-parallel-to-grain and other strength data per species, plus the orthotropic (L/R/T) framework for deriving grain-orientation anisotropy ratios used to scale Kc. Grade B source for deriving per-species wood Kc.

### B4. Plastics Kc — NO normalized kc1.1/mc exists; use research forces or datasheet estimation (Grade B)
A clean Sandvik-style kc1.1 + mc pair is **not published for any thermoplastic** (PMMA, HDPE, PC, POM). Polymer-machining papers report raw forces, force-per-width (N/mm), or specific cutting energy (J/mm³ = N/mm²) under specific conditions; Kc must be back-calculated as Kc ≈ Fc/(ap·f) or read as SCE. Best primary sources:
- **HDPE — Yang, Wang, Fu & Wang (2022),** "Prediction of Cutting Force and Chip Formation… for Polymer Machining," Polymers 14(1):189, DOI 10.3390/polym14010189 (open access). Orthogonal cutting (Kistler 9257B, carbide tool, 15°/30° rake, cut depths 60/120/180 µm). Reports specific cutting force Fc/b and Ft/b vs cut depth (Fig. 5) and derived cutting yield stress **33.85 MPa (30° rake) to 46.89 MPa (15° rake)** vs 27.4 MPa quasi-static; fracture toughness Gc ≈ 1.17–1.37 kJ/m². Hybrid FEM+experiment — force values are graphical.
- **POM-C — Trifunović et al. (2021),** "Investigation of cutting and specific cutting energy in turning of POM-C using a PCD tool," J. Cleaner Production 303:127043, DOI 10.1016/j.jclepro.2021.127043 (paywalled; abstract free). The single most on-target polymer SCE study — produces referential specific cutting energy (J/mm³) maps for POM-C; exact numbers behind paywall.
- **POM-C — Chabbi et al. (2017),** Measurement 95:99–115 (DOI 10.1016/j.measurement.2016.09.043) and Int. J. Adv. Manuf. Technol. 91:2267–2290 (DOI 10.1007/s00170-016-9858-8): tangential cutting force Fz regression models (carbide tool); back-calculate Kc = Fc/(ap·f). Paywalled.
- **PMMA — Korkmaz, Önler & Özdoğanlar (2017),** "Micromilling of PMMA Using Single-Crystal Diamond Tools," Procedia Manufacturing 10:683–693, DOI 10.1016/j.promfg.2017.07.017 (open access). Micromilling cutting forces vs spindle/feed/depth (Ø450 µm SCD endmill) — relevant to <3 mm tooling in plastics.
- **PC**: no measured-Kc study located; estimate from datasheet strength/hardness (see C).

Caveat/flag: for **plastics Kc**, the dataset should explicitly mark these as Grade B derived/estimated, cite the specific paper + figure, and note grade dependence; PC in particular has no primary machining-force source.

---

## C. Hardness Sources (Janka, Shore D, Brinell)

### C1. Janka hardness (wood) — Grade A
- **The Wood Database** (wood-database.com/wood-articles/janka-hardness and per-species profiles): Janka value (lbf and N) per species at 12% MC; defines the test (11.28 mm / 0.444" ball to half diameter). Primary go-to for per-species Janka.
- **USDA FPL Wood Handbook** (above): authoritative side-hardness data underpinning published Janka values; citeable for softwood/hardwood.
- **PreciseBits "Relative Wood Hardness Table"** (precisebits.com/reference/relative_hardness_table.htm): Janka in lbf and kN, framed explicitly for estimating feed rates by species — useful cross-link between hardness and feeds. Grade B/C.
- Cross-check lists: Bell Forest Products, Nova USA Wood, woodandshop.com PDF; Wikipedia "Janka hardness test" documents ASTM D1037/D143 method and caveats. Grade C.

### C2. Shore D / Rockwell hardness (plastics) — Grade A
- **MatWeb** (matweb.com): per-grade datasheets for Acrylic (cast/extruded/molded PMMA), Polycarbonate (molded/extruded), HDPE, and acetal/Delrin, with hardness and mechanical fields; click-through to original (un-rounded) values. Primary database. Caveat: some datasheets need free login; acrylic is typically reported in Rockwell M, not Shore D.
- **Supplier datasheets** (named, open, citeable):
  - HDPE — Direct Plastics PE300/HDPE: **Shore D 64** (ISO 868), tensile yield 25 MPa.
  - Polycarbonate — Treatstock PC datasheet: **Shore D 80** (ASTM D2240), tensile ultimate 72.4 MPa; Professional Plastics machine-grade PC datasheet: flexural 90 MPa.
  - Delrin/acetal — Alro Delrin/Acetron POM-H: **Shore D 86**, Rockwell M89/R122, tensile 75.8 MPa; Eagle Plastics Delrin: Rockwell M94.
  - Acrylic — Direct Plastics cast acrylic: tensile 70 MPa, flexural 115 MPa, ball-indentation 235 MPa; MakeItFrom PMMA: Rockwell M93. (Quote acrylic hardness as Rockwell M.)
- **Curbell Plastics, Boedeker, Professional Plastics, Interstate Plastics**: machining guides with per-plastic behavior, recommended tool geometry, and some Shore D values. Boedeker's "Plastic Machining Guidelines" hub (boedeker.com/Technical-Resources/Technical-Library/Plastic-Machining-Guidelines) links per-process (turning/milling/drilling/sawing) parameter tables by material. Curbell's milling guideline page is bot-protected (use search snippet or PDF). Grade B for machining behavior, Grade C for hardness cross-check.

### C3. Brinell hardness (aluminum) — Grade A
- **MatWeb / ASM Material Data Sheets** (asm.matweb.com): per-alloy/temper sheets. Aluminum 6061-T6 (asm.matweb.com, bassnum=ma6061t6) states "Hardness, Brinell **95** … 500 g load; 10 mm ball" (with Vickers 107, Rockwell B 60, UTS 310 MPa, yield 276 MPa). Also 6061-T651, 7075-T6, etc. Primary citeable source.
- Corroborating: nuclear-power.com gives 6061-T6 Brinell ≈ 95; MachineMFG aluminum hardness chart gives 6061-T6 ≈ HB 90–95 and 7075-T6 ≈ HB 150. Grade C cross-check; cite MatWeb/ASM as primary.

---

## Details: Gap-Closing Summary

**Plastics breadth (HDPE, PC, Delrin, acrylic):** LMT Onsrud (A2) is the decisive primary — its Soft Plastic and Hard Plastic data sheets tabulate HDPE, polycarbonate, extruded/cast acrylic, and nylon (acetal-class) with O-flute/upcut/downcut/compression geometry and chipload-by-CED, plus the verbatim 0.004"–0.012" optimum-chipload window for soft plastics/PC. Amana's Plastic O-Flute charts (A1) add a second Grade-A vendor. Curbell/Boedeker/Professional Plastics (C2) add per-material machining behavior and Shore D. Plastics **Kc** remains Grade B only — back-calculate from Yang (HDPE), Trifunović/Chabbi (POM), Korkmaz (PMMA); PC has no primary force study and must be estimated from datasheets.

**Small-diameter tooling (<3 mm):** Harvey Tool (A4) is the strongest — miniature square/ball/corner-radius/tapered with metric ball cutter diameters "down to just .05mm" and tapered cutters from 0.015", each with downloadable charts + MAP. Garr X3/G3 (A6, 1/32"–1/4") and Helical micro/tapered families (A5) corroborate. These also carry plastics and aluminum material columns, helping multiple gaps at once.

**Aluminum:** Garr aluminum milling guide (A6) gives SFM + chip-per-tooth-as-%-of-diameter + explicit ap/ae; Helical aluminum families (A5) and Amana ZrN Aluminum O-Flute (A1) give vendor charts; Sandvik N-group kc1.1 350–1350 N/mm² + mc (B1) and Kennametal (B2) supply aluminum Kc; MatWeb/ASM (C3) supply Brinell (6061-T6 = HB 95).

## Recommendations

1. **Ingest first, in this order (highest ROI Grade-A vendor charts):** (a) all Amana per-series PDFs from amanatool.com/pub/media/productattachments/ (script the file-name pattern; cite file + page 1/2); (b) every Onsrud per-material data sheet from onsrud.com/Forms/Cutting-Data-Recommendations.asp, OCR'ing the image-only PDFs (wood, some plastic sheets); (c) Harvey + Helical downloadable per-series charts for micro and tapered tools. This single pass populates the majority of the {tool family × material × diameter} grid with chipload, RPM (derive from the 18,000-RPM + formula), and ap rules.
2. **For Kc, split by material class:** metals/aluminum → Sandvik handbook kc1.1+mc and Kennametal (Grade A); wood/MDF/particleboard/plywood → Pałubicki 2021 + the HAL EWP coefficients paper + FPL Ch.5 shear data (Grade B); plastics → mark Grade B and store back-calculated Kc from the named papers, with PC explicitly flagged as estimated-from-datasheet.
3. **For hardness, set primary/secondary precedence:** wood Janka → Wood Database + FPL (primary), PreciseBits (feed-link cross-check); plastics → MatWeb + named supplier datasheets, storing Shore D for HDPE (64)/PC (80)/Delrin (86) and Rockwell M for acrylic (~M93 — do not force a Shore D onto acrylic); aluminum → MatWeb/ASM Brinell by alloy+temper.
4. **Record provenance per row:** source_url + source_page + evidence_grade, and a `derivation` note where RPM/feed were computed from a fixed-RPM chart or where Kc was back-calculated. Benchmarks that would change the plan: if Onsrud image PDFs resist OCR, fall back to the microfence.com Onsrud catalog mirror and the Scribd hard-plastic sheet; if a true plastics kc1.1/mc dataset surfaces (e.g., a future polymer Kienzle study), upgrade plastics Kc rows from Grade B to A.
5. **Use Grade-C sources (IDC, Carbide Processors, CNCCookbook, MachineMFG) only to sanity-check derived rows**, never as the sole citation for a row.

## Caveats
- **Plastics Kc is the dataset's structural weak point:** no published kc1.1/mc exists for PMMA/HDPE/PC/POM; values are back-calculated or estimated and must be graded B with explicit method notes. PC lacks any primary machining-force study.
- **Amana/Onsrud charts are fixed-RPM** (typically 18,000); the "RPM range" field must be derived from the chip-load formula and the machine envelope, not read directly.
- **Several Onsrud `/images/*.pdf` data sheets are image-only** (the wood sheet returned no machine-readable text) — OCR required; cite carefully.
- **Vendor charts are starting recommendations**, explicitly disclaimed by Amana/Onsrud/Harvey/Helical as needing test cuts; treat as conservative seeds.
- **Datasheet hardness/strength varies by grade and supplier** (e.g., HDPE tensile yield 22–31 MPa; PC Shore D ~80); always cite the specific named datasheet and note grade dependence.
- **Sandvik and Kennametal have removed standalone Kc charts from their live sites;** the citeable values now live in handbook/catalog PDFs (some via third-party mirrors), which should be archived at ingestion to preserve citeability.
- **Some MatWeb datasheets and the paywalled POM-C SCE paper require login/purchase**; budget for access or cite the open-access alternatives noted.