# Hardness Data Ingest — 2026-05-29

Citeable hardness data for `rs_cam` material classification / feeds-and-speeds.
Collected per source-acquisition doc `planning/feeds_data_source_acquisition_2026-05-29.md`
section C. **Every value below is read directly from a fetched source and stored
with a verbatim quote. No value is invented or estimated.** Where a source reports
a material in a particular hardness unit (e.g. acrylic in Rockwell M), that unit is
preserved — no cross-unit conversion was forced.

Evidence grade convention (from the acquisition doc):
- **Grade A**: direct manufacturer / authoritative-database datasheet value.
- **Grade B/C**: aggregator or feed-estimation cross-check table.

Touches no live code or data files. `material.rs` is left unmodified.

---

## 1. Janka Hardness — Wood (lbf at 12% MC, with N)

Test definition (The Wood Database):
> "the amount of pounds-force (lbf) or newtons (N) required to imbed a .444″ (11.28 mm) diameter steel ball into the wood to half the ball's diameter"
Ball 0.444 in (11.28 mm), embedded to half diameter, 12% MC.

FPL Wood Handbook defines the same quantity as side hardness:
> "side hardness is hardness measured when load is perpendicular to grain"

### 1a. Species the repo ALREADY has (`WoodSpecies::janka_lbf()`) — verification

| Species | Repo `janka_lbf()` | Wood Database (lbf / N) | PreciseBits (lbf) | FPL side hardness 12% (lbf) | Match? |
|---|---|---|---|---|---|
| RadiataPine | **500** | **710 lbf (3,150 N)** | 750 | — (not US-grown) | **DIFFERS** (repo low by ~210 lbf) |
| SouthernYellowPine | **690** | Longleaf 870 lbf (4,120 N) | Longleaf 870 | Loblolly/Longleaf family ~870 | **DIFFERS** (see note) |
| HardMaple (sugar) | **1450** | 1,450 lbf (6,450 N) | 1,450 | 1,450 (Maple, Sugar) | match |
| Walnut (black) | **1010** | 1,010 lbf (4,490 N) | 1,010 | 1,010 (Walnut, Black) | match |
| Birch | **1260** | Yellow 1,260 lbf (5,610 N) | Sweet 1,470 | Yellow 1,260 / Sweet 1,470 | match (yellow); PreciseBits used sweet |
| WhiteOak | **1360** | 1,350 lbf (5,990 N) | 1,360 | 1,360 (Oak, White → White) | match (1,350–1,360) |
| Jarrah | **1910** | 1,860 lbf (8,270 N) | — | — | **minor DIFF** (repo 1,910 vs WDB 1,860) |
| Ipe | **3510** | 3,490 lbf (15,520 N) | — | — | **minor DIFF** (repo 3,510 vs WDB 3,490) |
| GenericSoftwood | 600 | (not a species) | — | — | n/a (synthetic baseline) |
| GenericHardwood | 1450 | (not a species) | — | — | n/a (synthetic baseline) |

Verbatim quotes (Grade A — The Wood Database):
- Radiata Pine: `"Janka Hardness: 710 lbf (3,150 N)"`
- Longleaf Pine (SYP surrogate): `"870 lbf (4,120 N)"`
- Hard Maple: `"1,450 lbf (6,450 N)"`
- Black Walnut: `"Janka Hardness: 1,010 lbf (4,490 N)"`
- Yellow Birch: `"1,260 lbf (5,610 N)"`
- White Oak: `"1,350 lbf (5,990 N)"`
- Jarrah: `"1,860 lbf (8,270 N)"`
- Ipe: `"3,490 lbf (15,520 N)"`

PreciseBits Relative Wood Hardness Table (Grade B/C, framed for feed estimation):
> "If you know the optimum feed-rate to use on one species of wood, you can 'guesstimate' the feed-rate to use on another type of wood by comparing the Janka numbers."
- `pine, radiata (Monterey) | 3.3 kN | 750 lbf`
- `pine, longleaf | 3.9 kN | 870 lbf`
- `maple, sugar (hard) | 6.4 kN | 1,450 lbf`
- `walnut, black | 4.5 kN | 1,010 lbf`
- `birch, sweet | 6.5 kN | 1,470 lbf`
- `oak, white | 6.0 kN | 1,360 lbf`

FPL side-hardness, 12% MC (Grade A; verbatim row tails, units = lbf):
- Maple, Sugar 12%: `1,450`
- Oak, Northern red 12%: `1,290`
- Oak, White (white) 12%: `1,360`
- Walnut, Black 12%: `1,010`
- White Ash 12%: `1,320`
- Birch, Yellow 12%: `1,260`; Birch, Sweet 12%: `1,470`
- Cherry, black 12%: `950`
- Douglas-fir, Coast 12%: `710`; Interior West `660`; Interior North `600`
FPL metric (N) side-hardness, 12% MC: Sugar maple `6,400`; Northern red oak `5,700`; White oak `6,000`; Black walnut `4,500`.

**Discrepancy notes (do NOT edit material.rs — flagged for review):**
- **RadiataPine**: repo `500`, all three sources cluster `710–750`. Repo value looks low.
- **SouthernYellowPine**: repo `690`. "Southern Yellow Pine" is a trade group (longleaf/
  loblolly/shortleaf/slash); the closest single-species published Janka is Longleaf `870`.
  Repo `690` is below any SYP member's published Janka. Loblolly is typically ~690 in some
  aggregators, so the repo may be tracking loblolly — but no Grade-A loblolly Janka was
  fetched here (Longleaf only). Flagged as ambiguous, not corrected.
- **Jarrah** (1,910 vs 1,860) and **Ipe** (3,510 vs 3,490): repo values are within rounding /
  source-variation range of Wood Database; minor, likely a different source edition.
- HardMaple, Walnut, Birch (yellow), WhiteOak all match the repo within source rounding.

### 1b. Additional common species (NOT in repo — candidates to add)

| Species | Value (lbf) | Value (N) | Std/conditions | Grade | Source | Verbatim quote |
|---|---|---|---|---|---|---|
| Red Oak (northern) | 1,220 | 5,430 | 12% MC, side hardness | A | Wood Database `red-oak/` | `"1,220 lbf (5,430 N)"` (PreciseBits: 1,290; FPL: 1,290) |
| Black Cherry | 950 | 4,230 | 12% MC | A | Wood Database `black-cherry/` | `"950 lbf (4,230 N)"` (FPL: 950) |
| Yellow Poplar | 540 | 2,400 | 12% MC | A | Wood Database `poplar/` | `"540 lbf (2,400 N)"` (PreciseBits: 540) |
| White Ash | 1,320 | 5,870 | 12% MC | A | Wood Database `white-ash/` | `"1,320 lbf (5,870 N)"` (FPL: 1,320; PreciseBits: 1,320) |
| Honduran Mahogany | 900 | 4,020 | 12% MC, side grain | A | Wood Database `honduran-mahogany/` | `"900 lbf (4,020 N)"` (PreciseBits "mahogany, true": 800) |
| Douglas-Fir | 620 | 2,760 | 12% MC | A | Wood Database `douglas-fir/` | `"620 lbf (2,760 N)"` (PreciseBits coast: 710; FPL coast 710 / int. north 600) |

Sources:
- The Wood Database — https://www.wood-database.com/ (per-species + janka-hardness article)
- PreciseBits Relative Wood Hardness Table — https://www.precisebits.com/reference/relative_hardness_table.htm
- USDA FPL Wood Handbook GTR-190 Ch.5, Tables 5-3a (metric) / 5-3b (inch-pound) —
  https://research.fs.usda.gov/treesearch/download/37440.pdf
  (mirror: https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf)

**GAP — MDF / Particleboard surrogate Janka:** No Grade-A Janka exists for MDF or
particleboard (Janka is an ASTM D143 solid-wood test; engineered panels are not Janka-rated).
PreciseBits table explicitly omits them ("MDF and particleboard are not included in this table").
The repo's `SheetGoodKind::effective_janka_lbf()` (MDF 1100, HDF 1300, Particleboard 750) are
synthetic effective values, not citeable Janka. Logged in `hardness_gaps.md`.

---

## 2. Shore D / Rockwell — Plastics

Acrylic is stored as **Rockwell M**, NOT Shore D, per the overriding rule (source reports it
in Rockwell M).

| Material | Value | Unit | Test std | Grade | Source | Verbatim quote |
|---|---|---|---|---|---|---|
| HDPE (PE300, natural) | 64 | Shore D | ISO 868 (DIN/EN/ISO "868") | A | Direct Plastics HDPE datasheet | `"Shore hardness D | 64 | - | 868"` (table row) |
| HDPE (PE300, black PE-HWU) | 64 | Shore D | ISO 868 | A | Direct Plastics HDPE datasheet (p.2) | `"Shore hardness D | 64 | - | 868"` |
| Polycarbonate | 80 | Shore D | ASTM D2240 | A | Treatstock PC physical properties | `"Hardness, Shore D 80 80 ASTM D2240"` |
| Polycarbonate | 75 | Rockwell M | ASTM D785 | A | Treatstock PC physical properties | `"Hardness, Rockwell M 75 75 ASTM D785"` |
| Polycarbonate | 126 | Rockwell R | ASTM D785 | A | Treatstock PC physical properties | `"Hardness, Rockwell R 126 126 ASTM D785"` |
| Delrin / Acetron POM-H (acetal homopolymer) | 86 | Shore D | ASTM D2240 | A | Alro Delrin/Acetron POM-H datasheet | `"Hardness, Shore D 86 86 ASTM D2240"` |
| Delrin / Acetron POM-H | 89 | Rockwell M | ASTM D785 | A | Alro Delrin/Acetron POM-H datasheet | `"Hardness, Rockwell M 89 89 ASTM D785"` |
| Delrin / Acetron POM-H | 122 | Rockwell R | ASTM D785 | A | Alro Delrin/Acetron POM-H datasheet | `"Hardness, Rockwell R 122 122 ASTM D785"` |
| Acrylic (PMMA) | 93 | **Rockwell M** | (Rockwell M scale) | A | MakeItFrom PMMA | `"Rockwell M Hardness 93"` |

Notes:
- HDPE PE300 datasheet uses the DIN/EN/ISO column; the "868" entry on the "Shore hardness D"
  row is the ISO 868 standard reference. Both natural (PE-HWST, 0.947 g/cm³) and black
  (PE-HWU, 0.955 g/cm³) variants list Shore D 64. Matches the acquisition-doc target
  (HDPE Shore D 64, ISO 868).
- Polycarbonate datasheet (Treatstock, Gemini/Thermo Fab Plastics source) gives **both**
  Shore D 80 and Rockwell M 75. Matches acquisition-doc target (PC Shore D 80, ASTM D2240).
- Delrin matches acquisition-doc target exactly (Shore D 86 / Rockwell M89, Alro POM-H).
  Additional verbatim mechanical context: `"Tensile Strength 75.8 MPa ... ASTM D638"`.
- Acrylic: stored as **Rockwell M 93** (NOT converted to Shore D). Matches acquisition-doc
  target (MakeItFrom PMMA Rockwell M93).

Repo cross-reference: `PlasticFamily` enum has Acrylic, Hdpe, Delrin, Polycarbonate, Generic.
The repo currently models ALL plastics with a single flat `hardness_index() = 0.5` and
`kc = 4.0` regardless of family — it does NOT store per-family hardness. So these values are
purely additive reference data; no existing per-plastic hardness number to compare against.

Sources:
- Direct Plastics HDPE PE300 — https://www.directplastics.co.uk/pub/pdf/datasheets/HDPE%20Data%20Sheet.pdf
- Treatstock Polycarbonate — https://static.treatstock.com/static/fxd/wikiMaterials/polycarbonate/files/polycarbonate_physical_properties.pdf
- Alro Delrin/Acetron POM-H — https://www.alro.com/dataPDF/Plastics/TechnicalDataSheets/TDS_Delrin.pdf
- MakeItFrom PMMA — https://www.makeitfrom.com/material-properties/Polymethylmethacrylate-PMMA-Acrylic

---

## 3. Brinell Hardness — Aluminum

| Alloy/temper | Value | Unit | Conditions | Grade | Source | Verbatim quote |
|---|---|---|---|---|---|---|
| Aluminum 6061-T6 / T651 | 95 | HB (Brinell) | 500 g load; 10 mm ball | A | ASM / MatWeb (bassnum=ma6061t6) | `"Hardness, Brinell 95 95   AA; Typical; 500 g load; 10 mm ball"` |
| Aluminum 7075-T6 | 150 | HB (Brinell) | 500 g load; 10 mm ball | A | ASM / MatWeb (bassnum=ma7075t6) | `"Brinell 150 150 ... AA; Typical; 500 g load; 10 mm ball"` |

Supporting (same 6061-T6 ASM sheet, verbatim):
- `"Hardness, Vickers 107 107   Converted from Brinell Hardness Value"`
- `"Hardness, Rockwell B 60 60   Converted from Brinell Hardness Value"`
- `"Ultimate Tensile Strength 310 MPa 45000 psi   AA; Typical"`
- `"Tensile Yield Strength 276 MPa 40000 psi   AA; Typical"`
7075-T6 ASM sheet: `"Ultimate Tensile Strength 572 ..."`, `"Tensile Yield Strength 503 ..."`.

Both match acquisition-doc targets (6061-T6 Brinell 95 @ 500 g / 10 mm ball; 7075-T6 ≈ HB 150).

Repo cross-reference: `material.rs` has NO aluminum `Material` variant. Aluminum is
out-of-model today; these are additive reference values for any future aluminum support.

Source: ASM Aerospace Specification Metals / MatWeb data sheets — https://asm.matweb.com/
(fetched via curl with `-k` due to an intermediate-CA chain that WebFetch/curl could not
verify; content itself is the standard public ASM data sheet — see gaps note).

---

## Summary counts

- **Wood (Janka):** 14 entries — 8 repo-existing species verified + 6 additional species.
  - 2 clear discrepancies vs repo (RadiataPine 500 vs 710–750; SouthernYellowPine 690 vs
    Longleaf 870), 2 minor (Jarrah 1910 vs 1860; Ipe 3510 vs 3490), rest match.
- **Plastics (Shore D / Rockwell M):** 8 entries across 4 families (HDPE, PC, Delrin, Acrylic).
  Acrylic stored in Rockwell M per rule.
- **Aluminum (Brinell):** 2 alloys (6061-T6, 7075-T6).
- All values Grade A except PreciseBits (B/C cross-check) and FPL-as-corroboration.
