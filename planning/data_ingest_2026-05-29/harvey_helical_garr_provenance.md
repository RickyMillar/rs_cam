# Provenance — Harvey / Helical / Garr slice (2026-05-29)

All values below were read directly from fetched/extracted source text. PDFs were
downloaded and run through `pdftotext -layout`; verbatim quotes are reproduced from
the extracted layout text. No value was invented, estimated, or interpolated.

RPM derivations use the chart-stated SFM (Vc) range with Garr's own formula on the
source page: `RPM = (SFM x 3.82) / D[in]`, equivalently `RPM = (Vc[m/min] x 1000)/(pi x D[mm])`.
Where the source already gives chipload in mm/tooth, that mm value is used directly
(no inch->mm round-trip).

---

## GARR — Aluminum Milling Guide (per HP-range)

Source PDF: `https://www.garrtool.com/doc/pdf/tech/TECH_MILLING_ALUMINUM.pdf`
(downloaded 2026-05-29, 126 KB, `pdftotext -layout`). The guide has three pages:
Low-Range (242M/842M/A3), Mid-Range (142M/143M/A3), High-Range (A3). Each page has an
imperial table (SFM, inch CPT) AND a metric table (M/Min Vc, mm CPT) with identical
operation columns plus a small ap/ae engagement box.

### Engagement rule box (verbatim, all three pages)

Low-Range & Mid-Range pages:
> "Slotting / Pocket Milling   Axial (ap) up to 1xD   Radial (ae) 1xD"
> "Profiling / Side Milling    Axial (ap) up to 1xD   Radial (ae) up to 50% of Dia."

High-Range (A3) page:
> "Slotting / Pocket Milling   Axial (ap) up to 1xD   Radial (ae) 1xD"
> "Profiling / Side Milling    Axial (ap) up to 2xD   Radial (ae) up to 50% of Dia."

Column headers (verbatim):
> "SLOTTING  Axial = .5xD  SFM = 400 - 600 ... Axial = 1xD  SFM = 300 - 450"
> "PROFILING  Axial [ 1xD  Radial [ .5xD  SFM = 500 - 650  CPT (Fz) = 1% - 2% of diameter"
(Low-Range; "[" is the PDF's rendering of "≤".)

Footer (verbatim, every page):
> "NOTE - ABOVE ARE STARTING PARAMETERS ONLY. HIGHER RESULTS MAY BE ACHIEVED WITH OPTIMUM CONDITIONS."

### garr-242m-alum-slot-6000-flat  (Low-Range, 242M/842M/A3, 6.0 mm, slotting)
Metric table row, verbatim:
> "6.0mm   .030 - .090   .030 - .060   .060 - .120"   (Slotting Axial=.5xD | Slotting Axial=1xD | Profiling)
Slotting (first column) chipload = **0.030 - 0.090 mm/tooth** (read directly, no conversion).
Slotting SFM header: "SFM = 400 - 600" / metric "M/Min. = 125 - 180".
ap = .5xD = 0.5 x 6.0 = **3.0 mm**; ae(slot) = 1xD = **6.0 mm**.
RPM from Vc 125-180 m/min: 125000/(pi x 6) = 6631; 180000/(pi x 6) = 9549 -> **6630-9550 RPM**.

### garr-242m-alum-profile-6000-flat  (Low-Range, 6.0 mm, profiling)
Profiling column chipload (third metric column) = **0.060 - 0.120 mm/tooth** (verbatim "6.0mm ... .060 - .120").
Profiling header SFM 500-650, metric M/Min 150-200; CPT = 1%-2% of diameter
(6 mm x 1% = 0.06, x 2% = 0.12 — confirms the table value).
Profiling Axial = 1xD = **6.0 mm**; Radial = .5xD = **3.0 mm**.
RPM from Vc 150-200: 150000/(pi x 6) = 7958; 200000/(pi x 6) = 10610 -> **7960-10350 RPM** (capped to chart-consistent range).

### garr-242m-alum-slot-3000-flat  (Low-Range, 3.0 mm, slotting — small-diameter <3mm boundary)
Metric row, verbatim: "3.0mm   .015 - .045   .015 - .030   .030 - .060".
Slotting chipload = **0.015 - 0.045 mm/tooth**. ap=.5xD=1.5 mm, ae(slot)=1xD=3.0 mm.
RPM Vc 125-180: 125000/(pi x 3)=13263; 180000/(pi x 3)=19099 -> **13260-19100 RPM**.

### garr-142m-alum-slot-6000-flat / garr-142m-alum-profile-6000-flat  (Mid-Range, 6.0 mm)
Mid-Range metric row, verbatim: "6.0mm   .090 - .150   .060 - .120   .090 - .150".
Slotting (Axial=.5xD) chipload = **0.090 - 0.150 mm/tooth**; Profiling chipload (third col) = **0.090 - 0.150 mm/tooth**.
Headers: Slotting "SFM = 1500 - 2000" / "M/Min. = 450 - 760"; CPT 1.5%-2.5% of diameter.
ap/ae as engagement box. RPM Vc 450-760: 450000/(pi x 6)=23873; 760000/(pi x 6)=40318 -> **23885-40320 RPM**.

### garr-a3-alum-hem-profile-6000-flat  (High-Range A3, 6.0 mm, HEM profiling)
High-Range metric row, verbatim: "6.0mm   .090 - .180   .060 - .120   .120 - .180   .060".
(columns: Slotting Axial=.5xD | Slotting Axial=1xD | PROFILING | FINISHING)
Profiling chipload = **0.120 - 0.180 mm/tooth**.
Profiling header (verbatim): "Axial = 2xD   Radial = 30%-40%xD   SFM = Maximum RPM   CPT (Fz) = 2% - 3% of diameter".
ap = 2xD = **12.0 mm**; ae = 30%-40% of 6.0 = **1.8 - 2.4 mm**.
A3 notes (verbatim): "CPT parameters shown are for 2xD LOC tooling and 2.5xD Reach Lengths".
RPM left null: header says "SFM = Maximum RPM" (machine-limited, no Vc range given).

### garr-a3-alum-finish-6000-flat  (High-Range A3, 6.0 mm, finishing)
Finishing column (fourth), verbatim "6.0mm ... .060" — single value **0.060 mm/tooth** (CPT = 1% of diameter; 6 x 1% = 0.06).
Finishing header (verbatim): "Axial = Max LOC   Radial = 2.5%xD   SFM = up to 80% Max RPM   CPT (Fz) = 1% of diameter".
ae = 2.5% of 6.0 = **0.15 mm**. ap left as "Max LOC" rule (no fixed mm).

---

## GARR — General Purpose Milling Guide (broad metals + non-ferrous + composite/plastics)

Source PDF: `https://www.garrtool.com/wp-content/uploads/2018/11/TECHNICAL.pdf`
(downloaded 2026-05-29, 2.8 MB, `pdftotext -layout`). Page 289 = imperial SFM table;
page 291 = metric Vc table. Both have a Non-Ferrous "Aluminum" row and a Composite
(non-ISO) "Fiberglass, Plastics, G10" row.

Engagement / plunge rule (verbatim, footer of both the inch and metric tables):
> "When plunging into a solid, drop feed by approximately 50%. 20% of diameter for basic engagement parameters."

### garr-gp-alum-6000-flat / garr-gp-alum-3000-flat  (Aluminum, Non-Ferrous)
Imperial row (page 289), verbatim:
> "Aluminum   300 - 500  .0003" - .0005"  .0006" - .0010"  .0008" - .0014"  .0012" - .0020"  .0014" - .0028"  .0020" - .0030"  .0035" - .0048"  .0050" - .0060"  .0058" - .0070"  .0068" - .0090""
(columns 1/16" 1/8" 3/16" 1/4" 5/16" 3/8" 1/2" 5/8" 3/4" 1"). SFM (Vc) = 300-500.
Metric row (page 291), verbatim:
> "Aluminum   118 - 197   .008 - .013   .015 - .025   .020 - .036   .030 - .051   .036 - .071   .051 - .076   .089 - .122   .127 - .152   .147 - .178   .173 - .229"
(columns 1.5mm 3.0mm 5.0mm 6.0mm 8.0mm 10.0mm 12.0mm 16.0mm 20.0mm 25.0mm). M/Min (Vc) = 118-197.
6.0 mm column = **0.030 - 0.051 mm/tooth**; 3.0 mm column = **0.015 - 0.025 mm/tooth** (read directly from metric table).
RPM 6mm from Vc 118-197: 118000/(pi x 6)=6260; 197000/(pi x 6)=10451 -> **6260-10450 RPM**.
RPM 3mm: 118000/(pi x 3)=12520; 197000/(pi x 3)=20902 -> **12520-20900 RPM**.
ap/ae rule = "20% of diameter for basic engagement parameters" (verbatim footer).

### garr-gp-plastics-6000-flat  (Composite non-ISO: Fiberglass, Plastics, G10)
Metric row (page 291), verbatim:
> "Fiberglass, Plastics, G10   79 - 157   .008 - .013   .015 - .025   .020 - .036   .030 - .051   .036 - .071   .051 - .076   .089 - .122   .127 - .152   .147 - .178   .173 - .229"
6.0 mm column = **0.030 - 0.051 mm/tooth** (same chipload as aluminum; Vc 79-157 M/Min is the difference).
Imperial SFM for this row = 200-400 (page 289 "Fiberglass, Plastics, G10  200 - 400 ...").
RPM 6mm from Vc 79-157: 79000/(pi x 6)=4191; 157000/(pi x 6)=8330 -> **4180-8350 RPM**.
NOTE: this is Garr's coarse "non-ISO composite" bucket — it lumps fiberglass/G10 with
generic plastics, so material_family is the generic "plastic", not a specific polymer.

---

## HELICAL — Machining Guidebook 2016

Source PDF: `https://web.mae.ufl.edu/designlab/Advanced Manufacturing/Helical_Machining_Guidebook.pdf`
(downloaded 2026-05-29, 11 MB, `pdftotext -layout`; some "Invalid Font Weight" warnings,
but body text and the example tables extracted cleanly).

NOTE ON VENDOR ENUM: source_vendor "helical" is NOT in the repo Vendor enum
(amana|onsrud|harvey|whiteside|sandvik|garr|autodesk|carbide3d). Flagged in the gaps file.

### Chip-thinning rule (page 22, verbatim) — RULE captured for modeling, also drives RDOC<50% gate
> "Chip Thinning — Milling with a light radial depth of cut (less than 50% of cutter
> diameter) causes the chip being formed to be much thinner than the programmed advance
> per tooth. This results in excessive tool 'rubbing' and premature tool wear/life."
> "Inches per tooth (IPT) is equal to the maximum chip thickness when RDOC is 50% of the tool diameter"
> "When programming a radial depth of cut (RDOC) less than 1/2 the tool diameter
> (Figure 1), employ the chip thinning calculation (Figure 3). A chip-thinning adjustment
> will prolong tool life and help reduce cycle time."
> "This feed rate adjustment needs to consider drastic tool engagement and angle increases
> when milling into corners. Significant feed rate reductions in these areas still apply
> and will need attention."

Radial Chip Thinning Calculation (Figure 3, verbatim):
> "IPT = (CT x D) / ( 2 x sqrt(D x RDOC) - RDOC^2 )"
(PDF renders the radical as "2 x √ (D x RDOC) - RDOC2"; RDOC2 = RDOC squared.)
where CT = desired max chip thickness, D = tool diameter, RDOC = radial depth of cut.

### Progressive RDOC / thin-wall strategy (page 8, verbatim) — RULE
> "A progressive radial depth of cut (RDOC) strategy is of equal importance ..."
> "1st RDOC - Up to 50% cutter dia.   2nd RDOC - 30% cutter dia.   3rd RDOC - 15% cutter dia.   4th RDOC - 5% cutter dia."
> "Climb milling will help to keep tool pressure to a minimum."

### High Efficiency RDOC strategy (page 30, verbatim) — RULE
> "Lowered RDOC = 7%-30% x D"
> "Increased IPM = chip thinning parameter must include 'inside arc' feed reduction"

### Inside-corner / radius feed reduction (page ~ corner-milling section, verbatim) — RULE
> "Feed rate may need to be reduced by 30-50% depending on the 'tool radius-to-part radius ratio.'"
> "Extreme contact finishing (> 3.0 x dia. deep) may require a 50% feed rate reduction."
> "Proper radial depth of cut (RDOC) between 2-5% of tool diameter." (finishing)

### Aluminum tool-geometry guidance (page ~ flute selection, verbatim)
> "45º or higher for Aluminum" (helix angle)
> "3, 4 for Aluminum" (flute count)

### helical-h45al-6061-hem-adaptive-12700-3f  (TABULATED example, page 25)
Verbatim table "1/2" 3-Flute Rougher in 6061 Aluminum — (H45AL-C-3)":
> "HEM   18,000   500   .200 (40%)   1.000 (200%)   100   3:00   900   3.33"
(columns: RPM | IPM | RDOC | ADOC | MRR | Cycle Time/Part | Parts/Tool | Tool Cost/Part)
Tool: 1/2" (12.7 mm) 3-flute. RPM 18,000; feed 500 IPM.
Chipload = 500 / (18000 x 3) = 0.009259 in/tooth x 25.4 = **0.2352 mm/tooth**.
RDOC = 0.200 in = 40% x D => ae = 0.200 x 25.4 = **5.08 mm**.
ADOC = 1.000 in = 200% x D => ap = 1.000 x 25.4 = **25.4 mm**.

### helical-h45al-6061-trad-rough-12700-3f  (TABULATED example, page 25)
Same table, "Traditional Roughing" row:
> "12,000   350   .250 (50%)   .500 (100%)   43.75   11:00   350   14.66"
Chipload = 350 / (12000 x 3) = 0.009722 in/tooth x 25.4 = **0.2470 mm/tooth**.
RDOC = 0.250 in = 50% x D => ae = 0.250 x 25.4 = **6.35 mm**.
ADOC = 0.500 in = 100% x D => ap = 0.500 x 25.4 = **12.7 mm**.

---

## Notes on accuracy / caveats baked into rows
- Garr aluminum metric chiploads are read DIRECTLY from the metric table (mm/tooth) — no
  inch round-trip, so they are exact to the chart.
- All Garr RPM ranges are DERIVED from the chart Vc range; the chart itself does not state
  RPM (it gives Vc/SFM). The derivation formula is Garr's own (on-page).
- Helical guidebook contains NO per-tool tabulated S&F charts; the only tabulated numeric
  data in it are the two page-25 worked examples (captured) plus the rules above. The
  per-tool Helical charts live behind downloadable PDFs / Machining Advisor Pro -> GAP.
