# Vendor breadth provenance — Phase 3 / G beat (Whiteside / Freud / Vortex / IDC)

Accessed 2026-05-30. Every row in `vendor_breadth.json` cites a verbatim
source quote and the arithmetic used for unit conversion.

All inch→mm conversions: ×25.4. All chiplead back-calcs:
  fz = feedrate_ipm / (rpm × flutes), then ×25.4 → mm/tooth.

---

## G.1 — Whiteside Fusion 360 .tool Library (13 rows, Grade A, vendor-published)

**Source**: `Whiteside-Router-Bits-Fusion360.tools` (ZIP containing `tools.json`)
fetched from
https://www.dropbox.com/s/bqw4gqeggcv30o1/Whiteside-Router-Bits-Fusion360.tools?dl=1
(linked from https://www.whitesiderouterbits.com/pages/fusion-360-tool-files,
the official "Download Here: Fusion 360 .tool Library" link).

**Why Grade A**: This is a Whiteside-authored Autodesk Fusion 360 tool
library distributed via the vendor's official downloads page. Every record
contains the manufacturer's recommended starting feed/speed in the
`start-values.presets[0]` dict, with `f_z` (chip per tooth, inches),
`n` (RPM), `v_f` (feedrate, ipm), `geometry.NOF` (flute count), and
`geometry.DC` (cutter diameter, inches). The `vendor` field is
`"Whiteside Router Bits"` and `unit` is `"inches"`.

**Caveat**: Whiteside ships a single "Default Preset" per tool with no
per-material differentiation. The bits themselves target specific
materials per the product pages (RD5100 — general wood; UD2102 —
compression for plywood/melamine; etc.). Rows are tagged with the
typical-application material family from the Whiteside product page;
`material_label` notes the .tool file does not split per material.

### Row provenance (per tool):

#### RU1600 — 1/8" Upcut Spiral
- Verbatim from `tools.json`: `"product-id": "RU1600"`, `"description":
  "Whiteside RU1600 -- 1/8 inch Upcut Spiral - Solid Carbide"`, `"DC":
  0.125`, `"NOF": 2`, presets[0]: `{"f_z": 0.004, "n": 20000, "v_f":
  160}`.
- Arithmetic: D = 0.125 × 25.4 = 3.175 mm. fz = 0.004 × 25.4 = 0.1016 mm.
  Cross-check: 160 / (20000 × 2) = 0.004 in ✓.

#### RU1800 — 3/16" Upcut Spiral
- Verbatim: `"product-id": "RU1800"`, `"DC": 0.1875`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 0.1875 × 25.4 = 4.7625 mm. fz = 0.004 × 25.4 = 0.1016 mm.

#### RU2075 — 1/4" Upcut Spiral
- Verbatim: `"product-id": "RU2075"`, `"DC": 0.25`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 0.25 × 25.4 = 6.35 mm. fz = 0.1016 mm.

#### RD1600 — 1/8" Downcut Spiral
- Verbatim: `"product-id": "RD1600"`, `"DC": 0.125`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 3.175 mm. fz = 0.1016 mm.

#### RD1800 — 3/16" Downcut Spiral
- Verbatim: `"product-id": "RD1800"`, `"DC": 0.1875`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 4.7625 mm. fz = 0.1016 mm.

#### RD2075 — 1/4" Downcut Spiral
- Verbatim: `"product-id": "RD2075"`, `"DC": 0.25`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 6.35 mm. fz = 0.1016 mm.

#### RD5100 — 1/2" Upcut Spiral
- Verbatim: `"product-id": "RD5100"`, `"description": "Whiteside RD5100
  -- 1/2 inch Upcut Spiral - Solid Carbide"`, `"DC": 0.49999999999999994`
  (1/2 exact), `"NOF": 2`, `{"f_z": 0.011111111111111112, "n": 18000,
  "v_f": 400}`.
- D = 0.5 × 25.4 = 12.7 mm. fz = (1/90) × 25.4 ≈ 0.2822 mm. Cross-check:
  400 / (18000 × 2) = 0.01111 in ✓.

#### UD2102 — 1/4" Compression Spiral
- Verbatim: `"product-id": "UD2102"`, `"description": "Whiteside UD2102
  -- 1/4 inch Compression Spiral - Solid Carbide"`, `"DC": 0.25`,
  `"NOF": 2`, `{"f_z": 0.01125, "n": 20000, "v_f": 450}`.
- D = 6.35 mm. fz = 0.01125 × 25.4 = 0.2857 mm. Cross-check:
  450 / (20000 × 2) = 0.01125 ✓.

#### C1072 — 1/2" Straight Flute (Carbide Tipped)
- Verbatim: `"product-id": "C1072"`, `"description": "Whiteside C1072
  -- 1/2 inch Straight Flute - Carbide Tipped"`, `"DC": 0.5`,
  `"NOF": 2`, `{"f_z": 0.00833, "n": 18000, "v_f": 300}`.
- D = 12.7 mm. fz = 0.00833 × 25.4 ≈ 0.2117 mm.

#### RU1800RN — 3/16" Ball Nose Spiral
- Verbatim: `"product-id": "RU1800RN"`, `"type": "ball end mill"`,
  `"DC": 0.1875`, `"NOF": 2`, `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 4.7625 mm. fz = 0.1016 mm.

#### RU2075RN — 1/4" Ball Nose Spiral
- Verbatim: `"product-id": "RU2075RN"`, `"type": "ball end mill"`,
  `"DC": 0.25`, `"NOF": 2`, `{"f_z": 0.004, "n": 20000, "v_f": 160}`.
- D = 6.35 mm. fz = 0.1016 mm.

#### SC64 — 11° included Conical Ball Nose Spiral
- Verbatim: `"product-id": "SC64"`, `"description": "Whiteside SC64 --
  11 degree included Conical Ball Nose Spiral - Solid Carbide"`,
  `"type": "tapered mill"`, `"DC": 0.0568`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`. Per the Whiteside CNC
  brochure: SC64 is a tapered ball-tip with 1/16" ball dia (1.5875 mm)
  and 11° included angle.
- D (cutter shank/major) = 0.0568 × 25.4 ≈ 1.442 mm. fz = 0.1016 mm.
  tip_diameter_mm = 1/16" × 25.4 = 1.5875.

#### SC66 — 7° included Conical Ball Nose Spiral
- Verbatim: `"product-id": "SC66"`, `"description": "Whiteside SC66 --
  7 degree included Conical Ball Nose Spiral - Solid Carbide"`,
  `"type": "tapered mill"`, `"DC": 0.1176`, `"NOF": 2`,
  `{"f_z": 0.004, "n": 20000, "v_f": 160}`. Per the Whiteside CNC
  brochure: SC66 is a tapered ball-tip with 1/8" ball dia (3.175 mm)
  and 7° included angle.
- D (cutter major) = 0.1176 × 25.4 ≈ 2.987 mm. fz = 0.1016 mm.
  tip_diameter_mm = 1/8" × 25.4 = 3.175.

---

## G.2 — Freud Router Bit Feed Rates and Speeds (14 rows, Grade A, **NEW vendor enum needed**)

**Source**: `freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf`
fetched from
https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf
(linked from https://www.freudtools.com/downloads under the Router Bits
category as "Router Bit Feed Rates and Speeds").

**Why Grade A**: Freud-authored PDF distributed via the vendor's
official downloads page. Title: "Get the most out of your Freud bits by
routing at ideal feed rates and speeds".

**Verbatim CHIP LOADS table for Freud SOLID CARBIDE ROUTER BITS ONLY**
(page 2 of PDF):

```
CHIP LOADS FOR FREUD SOLID CARBIDE ROUTER BITS ONLY:
Recommended* Chip Loads, based on cut depth equal to bit diameter:
                MDF/    Laminated                                          Solid
   Tool        Particle  Particle                              Acrylics/ Surface/
 Diameter      Board     Board     Hardwood Softwood Soft Plastic Hard Plastic Plywood   Aluminum
   1/8"     .004"-.007" .003"-.006" .002"-.005" .004"-.006" .003"-.005" .002"-.004" .003"-.005" .003"-.004"
   1/4"     .013"-.017" .010"-.015" .008"-.011" .010"-.012" .006"-.009" .005"-.008" .006"-.009" .005"-.007"
   3/8"     .018"-.021" .014"-.018" .014"-.016" .016"-.019" .010"-.012" .008"-.010" .015"-.018" .006"-.008"
   1/2"     .023"-.027" .022"-.026" .018"-.021" .020"-.023" .012"-.015" .010"-.012" .018"-.021" .008"-.010"
```

Verbatim AP rule (page 1):
> "If Cut Depth is 2X the bit diameter, reduce the Chip Load by at least 25%
>  If Cut Depth is 3X the bit diameter, reduce the Chip Load by at least 50%"

Set `ap_rule` accordingly on every row. Flute count defaulted to 2 (most
Freud solid carbide router bits are 2-flute; the chart is per-diameter
not per-flute-count, and Freud's example #1 in the same PDF: "the bit
has 2 flutes (cutting edges)").

### Row-by-row arithmetic (inch ranges → mm, ×25.4):

| observation_id | inch range | mm range |
|---|---|---|
| freud-solid-carbide-eighth-hardwood | .002"-.005" | 0.0508-0.127 |
| freud-solid-carbide-quarter-hardwood | .008"-.011" | 0.2032-0.2794 |
| freud-solid-carbide-three-eighth-hardwood | .014"-.016" | 0.3556-0.4064 |
| freud-solid-carbide-half-hardwood | .018"-.021" | 0.4572-0.5334 |
| freud-solid-carbide-eighth-softwood | .004"-.006" | 0.1016-0.1524 |
| freud-solid-carbide-quarter-softwood | .010"-.012" | 0.254-0.3048 |
| freud-solid-carbide-half-softwood | .020"-.023" | 0.508-0.5842 |
| freud-solid-carbide-eighth-mdf-particle | .004"-.007" | 0.1016-0.1778 |
| freud-solid-carbide-quarter-mdf-particle | .013"-.017" | 0.3302-0.4318 |
| freud-solid-carbide-half-mdf-particle | .023"-.027" | 0.5842-0.6858 |
| freud-solid-carbide-quarter-plywood-hardwood | .006"-.009" | 0.1524-0.2286 |
| freud-solid-carbide-half-plywood-hardwood | .018"-.021" | 0.4572-0.5334 |
| freud-solid-carbide-quarter-hard-plastic | .005"-.008" | 0.127-0.2032 |
| freud-solid-carbide-quarter-aluminum | .005"-.007" | 0.127-0.1778 |

The "Acrylics / Hard Plastic" column is mapped to material_family
`acrylic` (the rs_cam enum has acrylic but not a separate "hard
plastic" — acrylic is the closest match within the chart's grouping).
"Plywood" mapped to `plywood_hardwood` per repo convention; the chart
does not separate plywood-softwood vs plywood-hardwood. MDF/Particle
Board → `mdf`. NB: the validator flags chipload >0.5 mm as suspicious;
these values are verbatim from the Freud chart (the chart's 1/2" rows
genuinely go up to .027" = 0.6858 mm for MDF) — they are NOT data
errors, but real high values for large-diameter solid-carbide bits in
softer materials. The values are honest; the validator threshold
is conservative.

---

## G.3 — Vortex Tool — chipload chart unavailable as machine-readable text (0 rows)

Catalog title: "Vortex Tool Selection Guide" — `Vortex_Catalog.pdf`,
fetched from
https://www.vortextool.com/media/assets/Vortex_Catalog.pdf (linked from
the home page footer "Download Wood Tooling Catalog").

The dedicated chip-load chart at
https://www.vortextool.com/media/assets/chipLoadChart.pdf was fetched
successfully (6.2 MB single-page PDF) but is an Adobe Photoshop-rendered
raster image — `pdfimages` extracts only one 3212×2260 JPEG, and no
text layer is present (`pdftotext` returns only garbled fragments).

The catalog itself contains a "Chip Load Chart" page (p.14) which is
also embedded as a graphic only. No machine-readable chipload data
could be extracted from any Vortex source.

OCR was not attempted because `tesseract` is not installed on the
collection host. Logged to gaps.

Vortex catalog product pages DO provide series-by-series CED/CEL/SHK
DIA/OAL geometry (verified) but DO NOT carry per-material chipload
values — those are isolated to the image-only chart.

---

## G.4 — IDC Woodcraft Feeds & Speeds Database (10 rows, Grade C, **NEW vendor enum needed**)

**Source**: `https://feeds-speeds-chipload-api.fly.dev/download-csv`,
linked from https://idcwoodcraft.com/pages/database-downloads. Title:
"Database Downloads — IDC Woodcraft". One CSV with 79 rows covering
IDC Woodcraft (63) and Cadence Manufacturing and Design (16) tooling.

**Why Grade C**: IDC Woodcraft is a community-leaning vendor whose
chipload database is aggregated from operator experience and the IDC
product line. Treat as cross-check against vendor-published data; not
suitable as a primary safety-gate source.

CSV columns include `vendor`, `model`, `diameter` (inches),
`numflutes`, `feedrate` (in/min), `rpm`. Chipload back-calc:
`fz_in = feedrate / (rpm × numflutes)`, then ×25.4 → mm/tooth.
`material` field on each row contains a JSON-blob with material
metadata; rows are mapped to the closest rs_cam `material_family`
based on the IDC product naming (OF = O-flute for acrylic, CM =
compression for plywood/melamine, BN = ball nose, etc.).

10 cleanest IDC-line rows selected (rejected: rows with rpm=0,
diameter=0.005 (clearly an IDC catalog typo on RO-18/RO-14), and very-
small `DC-132` 1/32" detail bits whose fz=0.005 mm/tooth is plausible
but extreme; kept only mainstream sizes).

### Row provenance:

- **OF-18** (1/8" Acrylic O Flute): CSV row `{"vendor":"IDC
  Woodcraft","model":"OF-18","diameter":0.125,"numflutes":1,
  "feedrate":60,"rpm":17000}`. fz = 60 / (17000×1) = 0.003529 in ×
  25.4 = 0.0896 mm/tooth. D = 0.125 × 25.4 = 3.175 mm.
- **OF-14** (1/4" Acrylic O Flute): CSV row `{"model":"OF-14",
  "diameter":0.25,"numflutes":1,"feedrate":60,"rpm":17000}`.
  fz = 60 / 17000 = 0.003529 in = 0.0896 mm. D = 6.35 mm.
- **CM-18** (1/8" Compression): CSV row `{"model":"CM-18",
  "diameter":0.125,"numflutes":2,"feedrate":50,"rpm":16000}`.
  fz = 50 / 32000 = 0.00156 in = 0.0397 mm. D = 3.175 mm.
- **CM-14** (1/4" Compression): CSV row `{"model":"CM-14",
  "diameter":0.25,"numflutes":2,"feedrate":80,"rpm":16000}`.
  fz = 80 / 32000 = 0.0025 in = 0.0635 mm. D = 6.35 mm.
- **DC-18** (1/8" Downcut Spiral): CSV row `{"model":"DC-18",
  "diameter":0.125,"numflutes":2,"feedrate":50,"rpm":22000}`.
  fz = 50 / 44000 = 0.00114 in = 0.0289 mm. D = 3.175 mm.
- **UC-18** (1/8" Upcut Spiral): CSV row `{"model":"UC-18",
  "diameter":0.125,"numflutes":2,"feedrate":50,"rpm":22000}`.
  fz = 0.0289 mm. D = 3.175 mm.
- **BN-18** (1/8" Ball Nose): CSV row `{"model":"BN-18","type":"ball",
  "diameter":0.125,"numflutes":2,"feedrate":60,"rpm":22000}`.
  fz = 60 / 44000 = 0.00136 in = 0.0346 mm. D = 3.175 mm.
- **BN-14** (1/4" Ball Nose): CSV row `{"model":"BN-14","type":"ball",
  "diameter":0.25,"numflutes":2,"feedrate":70,"rpm":19000}`.
  fz = 70 / 38000 = 0.00184 in = 0.0468 mm. D = 6.35 mm.
- **BN-12** (1/2" Ball Nose): CSV row `{"model":"BN-12","type":"ball",
  "diameter":0.5,"numflutes":2,"feedrate":60,"rpm":16000}`.
  fz = 60 / 32000 = 0.00187 in = 0.0476 mm. D = 12.7 mm.
- **SU-10** (1" Surfacing): CSV row `{"model":"SU-10","diameter":1.0,
  "numflutes":4,"feedrate":150,"rpm":10000}`.
  fz = 150 / 40000 = 0.00375 in = 0.0952 mm. D = 25.4 mm.

These rows give cross-reference fz for the small (1/8"–1/4") size range
that vendor-published charts often gloss, plus a 1" facing bit anchor
for surfacing.

---

## Summary

| Sub-target | Rows | Grade | Enum status |
|---|---|---|---|
| G.1 Whiteside Fusion 360 .tool | 13 | A | already in enum |
| G.2 Freud Solid Carbide chart | 14 | A | **needs `Vendor::Freud` variant** |
| G.3 Vortex chipload chart | 0 | — | image-only; OCR unavailable |
| G.4 IDC Woodcraft CSV | 10 | C | **needs `Vendor::Idcwoodcraft` (or similar) variant** |
| **Total** | **37** | | |

Validator: 13 Whiteside rows pass schema-clean; 24 Freud + IDC rows
flagged "vendor not in enum" (expected, logged in `_gaps.md`).
