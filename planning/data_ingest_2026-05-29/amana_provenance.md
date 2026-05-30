# Amana Tool — Provenance for data_ingest_2026-05-29/amana.json

All PDFs fetched 2026-05-29 via `curl` with a browser User-Agent (the direct
WebFetch path returned HTTP 403; curl + UA returned HTTP 200). Text extracted
with `pdftotext -layout`. Every value below is a verbatim transcription of the
extracted chart text. Conversions: chipload inch -> mm = inch x 25.4;
diameter inch -> mm = inch x 25.4.

All five charts state "CNC Operating Spindle Speed: 18,000 RPM" (fixed), so no
RPM range was read off the charts; rpm_min/max/nominal are left null and the
fixed 18,000 RPM + DOC rule are recorded in `machine_assumption`.

---

## Source: amana_ams159_vgroove_v2
URL: https://www.amanatool.com/pub/media/productattachments/AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf
Page 1. Title: "Solid Carbide and Carbide Tipped 18°, 30°, 45°, 60° & 90° Degree V-Groove Router Bits".
Header: "Operating RPM: 18,000 / Depth of Cut: 1 x Tool Diameter".
Tool reference row confirms 18/30/45 deg = Solid Carbide **1 Flute**; 60/90 deg = Carbide Tipped **2 Flute**.

Verbatim table rows (Feed Rate IPM* / Chip Load Per Tooth IPR** by angle):

```
                       18°                 30°                 45°            60°          90°
Soft Wood    50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  90" 0.003"  90" 0.003"
Hard Wood    50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  90" 0.003"  90" 0.003"
Soft Plastic 50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  90" 0.003"  90" 0.003"
Hard Plastic 50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  90" 0.003"  90" 0.003"
Aluminum     50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  N/A N/A     N/A N/A
Solid Surface 50"-130" 0.003"-0.007" 50"-130" 0.003"-0.007"  50"-130" 0.003"-0.007"  N/A N/A     N/A N/A
```

DOC rule verbatim: "Depth of Cut: 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%".

Conversions (1-flute angles 18/30/45, all listed materials): 0.003" x 25.4 = 0.0762 mm; 0.007" x 25.4 = 0.1778 mm.
Conversions (2-flute angles 60/90, wood + plastic): single 0.003" x 25.4 = 0.0762 mm (recorded as both min and max).

Rows derived:
- `amana-vbit-softwood-trace-18deg-1f` — Soft Wood, 18°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-hardwood-trace-18deg-1f` — Hard Wood, 18°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-softwood-trace-30deg-1f` — Soft Wood, 30°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-hardwood-trace-30deg-1f` — Hard Wood, 30°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-acrylic-trace-30deg-1f` — "Hard Plastic" row, mapped to acrylic-class, 30°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-softwood-trace-45deg-1f` — Soft Wood, 45°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-hardwood-trace-45deg-1f` — Hard Wood, 45°, 1F, 0.0762-0.1778 mm.
- `amana-vbit-aluminum-trace-45deg-1f` — Aluminum, 45°, 1F, 0.0762-0.1778 mm (Aluminum N/A at 60/90°, present at 18/30/45°).
- `amana-vbit-softwood-trace-60deg-2f` — Soft Wood, 60°, 2F, single 0.0762 mm.
- `amana-vbit-hardwood-trace-60deg-2f` — Hard Wood, 60°, 2F, single 0.0762 mm.
- `amana-vbit-softwood-trace-90deg-2f-ams159` — Soft Wood, 90°, 2F, single 0.0762 mm. (Distinct source from the pre-existing Insert V-Groove v16 90° rows; `-ams159` suffix avoids id collision.)

Note: only a subset of material rows transcribed into observations to avoid
redundant 1F/0.003-0.007" duplicates; the full table is identical across Soft
Wood / Hard Wood / Soft Plastic / Hard Plastic / Aluminum / Solid Surface for
the 18/30/45° columns and across Soft/Hard Wood + Soft/Hard Plastic for 60/90°.

---

## Source: amana_spektra_engraving_v4
URL: https://www.amanatool.com/pub/media/productattachments/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf
Page 1. Title: "Solid Carbide Spektra™ Extreme Tool Life Coated 15°, 30°, 45° & 120° Degree Single Flute Engraving Router Bits".
Header: "CNC Operating Spindle Speed: 18,000 RPM / Depth of Cut: 1 x Tool Diameter". All columns are **single flute**.
Tip widths from column headers: 15° = (Tip Width) 0.005"; 30° = (Tip Width) 0.005"-0.030"; 45° = (Tip Width) 0.042"; 120° = (Tip Width) 0.015".

Verbatim table rows (Feed Rate IPM* / Chip Load Per Tooth IPR**):

```
                15° (0.005")        30° (0.005"-0.030")  45° (0.042")        120° (0.015")
Soft Wood     50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Hard Wood     50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Soft Plastic  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Hard Plastic  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Solid Surface 50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
```

DOC rule verbatim: "Depth of Cut: 1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / 3 x D Reduce feed rate by 50%".

Conversions (15/30/45°): 0.003" x 25.4 = 0.0762 mm; 0.007" x 25.4 = 0.1778 mm.
Conversions (120°): 0.002" x 25.4 = 0.0508 mm; 0.006" x 25.4 = 0.1524 mm.
Tip-width conversions: 0.005" x 25.4 = 0.127 mm; 0.042" x 25.4 = 1.0668 mm; 0.015" x 25.4 = 0.381 mm.

Rows derived:
- `amana-engrave-softwood-trace-15deg-1f` — Soft Wood, 15°, tip 0.127 mm, 0.0762-0.1778 mm.
- `amana-engrave-hardwood-trace-15deg-1f` — Hard Wood, 15°, tip 0.127 mm, 0.0762-0.1778 mm.
- `amana-engrave-softwood-trace-30deg-1f` — Soft Wood, 30°, 0.0762-0.1778 mm.
- `amana-engrave-hardwood-trace-30deg-1f` — Hard Wood, 30°, 0.0762-0.1778 mm.
- `amana-engrave-softwood-trace-45deg-1f` — Soft Wood, 45°, tip 1.0668 mm, 0.0762-0.1778 mm.
- `amana-engrave-hardplastic-trace-45deg-1f` — "Hard Plastic" row, mapped to acrylic-class, 45°, tip 1.0668 mm, 0.0762-0.1778 mm.
- `amana-engrave-softwood-trace-120deg-1f` — Soft Wood, 120°, tip 0.381 mm, 0.0508-0.1524 mm.
- `amana-engrave-hardwood-trace-120deg-1f` — Hard Wood, 120°, tip 0.381 mm, 0.0508-0.1524 mm.

---

## Source: amana_compression_spirals_v8
URL: https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Compression-Spirals-v8.pdf
Page 1. Title: "Solid Carbide Compression Spiral Router Bits".
Header: "CNC Operating Spindle Speed: 18,000 RPM / Depth of Cut: 1 x Tool Diameter".
Columns: Wood | MDF/Laminate | Plywood | Plastic, each with Feed Rate IPM* / Chip Load Per Tooth / Ramp Down.

Verbatim **2 Flute** table rows used (Diameter / Wood / MDF-Laminate / Plywood / Plastic, each = Feed IPM, ChipLoad, RampDown):

```
1/8"   40"  .0011" 20"   80"  .0022" 40"   40"  .0011" 20"   40"  .0011" 20"
5/32"  60"  .0017" 30"  110"  .0031" 55"   60"  .0017" 30"   60"  .0017" 30"
3/16"  80"  .0022" 40"  160"  .0044" 80"   80"  .0022" 40"   80"  .0022" 40"
1/4"  110"  .0031" 55"  220"  .0061" 110"  110" .0031" 55"   110" .0031" 55"
3/8"  200"  .0056" 100" 400"  .0111" 200"  200" .0056" 100"  200" .0056" 100"
1/2"  280"  .0077" 140" 400"  .0111" 200"  280" .0077" 140"  280" .0077" 140"
```

DOC rule verbatim: "Depth of Cut: 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%".

Conversions (1/4" 2F): Wood/Plywood/Plastic .0031" x 25.4 = 0.0787 mm; MDF .0061" x 25.4 = 0.1549 mm.
Conversions (1/2" 2F): Wood/Plywood/Plastic .0077" x 25.4 = 0.1956 mm; MDF .0111" x 25.4 = 0.2819 mm.
Diameter conversions: 1/4" = 0.25" x 25.4 = 6.35 mm; 1/2" = 0.5" x 25.4 = 12.7 mm.

Rows derived (2-flute; mapped to flat_end / compression family):
- `amana-compression-wood-pocket-6350-2f` — Wood, 1/4" (6.35 mm), 2F, 0.0787 mm (Wood -> hardwood material_family).
- `amana-compression-mdf-pocket-6350-2f` — MDF/Laminate, 1/4" (6.35 mm), 2F, 0.1549 mm.
- `amana-compression-plywood-hardwood-pocket-6350-2f` — Plywood, 1/4" (6.35 mm), 2F, 0.0787 mm.
- `amana-compression-acrylic-pocket-6350-2f` — Plastic column, 1/4" (6.35 mm), 2F, 0.0787 mm (mapped to acrylic-class).
- `amana-compression-mdf-pocket-12700-2f` — MDF/Laminate, 1/2" (12.7 mm), 2F, 0.2819 mm.
- `amana-compression-wood-pocket-12700-2f` — Wood, 1/2" (12.7 mm), 2F, 0.1956 mm.

Note on material mapping: the chart "Wood" column is generic; recorded as
`hardwood` (the conservative/stiffer wood) with material_label "Wood
(compression spiral)". "Plywood" mapped to plywood_hardwood; "Plastic" mapped
to acrylic. Each chart cell is a single value (no range), so min == max.

---

## Source: amana_plastic_oflute_v2
URL: https://www.amanatool.com/pub/media/productattachments/Plastic-O-Flute-Speed-Chart-v2.pdf
Page 1. Title: "Solid Carbide Plastic Cutting Spiral Single 'O' Flute Router Bits". Single flute.
Columns: Diameter | IPM at 18,000 RPM | Spindle Speed SFM | Chip Load Per Tooth.
This chart is **by diameter only** — one chip-load column applies to all plastics (no per-plastic breakout).

Verbatim rows used:

```
1/8" (0.125)   70 - 110   500 - 1,200   0.004" - 0.006"
1/4" (0.250)  145 - 220   500 - 1,200   0.008" - 0.012"
```

(Full table also includes: 1/16" 0.002-0.004; 2mm 0.002-0.004; 3/32" 0.003-0.005;
3mm 0.004-0.006; 5/32"+4mm+3/16"+5mm 0.006-0.008; 6mm+9/32" 0.008-0.012;
5/16"+8mm 0.009-0.013; 21/64"+11/32" 0.010-0.014; 9mm+3/8"+10mm 0.011-0.016;
12mm+1/2" 0.015-0.020.)

DOC rule verbatim: "Depth of Cut: 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%".

Conversions: 1/8" 0.004" x 25.4 = 0.1016 mm, 0.006" x 25.4 = 0.1524 mm.
1/4" 0.008" x 25.4 = 0.2032 mm, 0.012" x 25.4 = 0.3048 mm.
Diameter: 1/8" = 3.175 mm; 1/4" = 6.35 mm.

Rows derived (single plastic column applied to acrylic / hdpe / polycarbonate;
material_label notes "(O-flute, all plastics)"):
- `amana-plastic-oflute-acrylic-pocket-3175-1f` — 1/8" (3.175 mm), 0.1016-0.1524 mm.
- `amana-plastic-oflute-hdpe-pocket-3175-1f` — 1/8" (3.175 mm), 0.1016-0.1524 mm.
- `amana-plastic-oflute-polycarbonate-pocket-3175-1f` — 1/8" (3.175 mm), 0.1016-0.1524 mm.
- `amana-plastic-oflute-acrylic-pocket-6350-1f` — 1/4" (6.35 mm), 0.2032-0.3048 mm.
- `amana-plastic-oflute-hdpe-pocket-6350-1f` — 1/4" (6.35 mm), 0.2032-0.3048 mm.

Caveat: the chart does NOT distinguish chipload by plastic type; the same
diameter-keyed value is the source's recommendation for all plastics. The
acrylic/hdpe/polycarbonate rows therefore share identical numbers and quotes —
this is faithful to the source, not an interpolation.

---

## Source: amana_zrn_aluminum_oflute_v13
URL: https://www.amanatool.com/pub/media/productattachments/ZrN-Aluminum-O-Flute-Speed-Chart-v13.pdf
Page 1. Title: "ZrN Coated Solid Carbide Aluminum Cutting Spiral Single 'O' Flute Router Bits". Single flute.
Header: "CNC Operating Spindle Speed: 18,000 RPM / Depth of Cut: 1 x Tool Diameter".
Columns: Diameter | IPM at 18,000 RPM | Spindle Speed SFM | Chip Load Per Tooth.

Verbatim rows used:

```
1/8" (0.125)    35" - 70"    600 - 1,000   0.002" - 0.004"
1/4" (0.250)    55" - 110"   600 - 1,000   0.003" - 0.006"
```

(Full table: 1/32"+1/16"+3/32"+1/8" all 0.002-0.004; 3/16"+1/4"+5/16" all 0.003-0.006.)

DOC rule verbatim: "Depth of Cut: 1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / 3 x D Reduce feed rate by 50%".

Conversions: 1/8" 0.002" x 25.4 = 0.0508 mm, 0.004" x 25.4 = 0.1016 mm.
1/4" 0.003" x 25.4 = 0.0762 mm, 0.006" x 25.4 = 0.1524 mm.
Diameter: 1/8" = 3.175 mm; 1/4" = 6.35 mm.

Rows derived:
- `amana-zrn-alum-oflute-aluminum-adaptive-3175-1f` — aluminum, 1/8" (3.175 mm), 1F, 0.0508-0.1016 mm.
- `amana-zrn-alum-oflute-aluminum-adaptive-6350-1f` — aluminum, 1/4" (6.35 mm), 1F, 0.0762-0.1524 mm.
