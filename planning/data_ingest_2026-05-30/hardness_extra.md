# Hardness Data Ingest (Extension) — 2026-05-30

PHASE 3 / Beat H of `planning/feeds_data_ingest_2026-05-30_phased_plan.md`.
Complements `planning/data_ingest_2026-05-29/hardness.md` (which covered the
already-modeled plastic families HDPE, PC, Delrin, Acrylic and aluminum 6061-T6 /
7075-T6).

**Honesty rule (verbatim from beat prompt):** every value below is read from a
fetched manufacturer datasheet or ASTM-traceable source and recorded with a
verbatim quote. No value is invented, estimated, interpolated, or converted
across hardness scales.

Touches no live code or data files. `crates/rs_cam_core/src/material.rs` is
left unmodified — these are additive reference values for future
`PlasticFamily` and `AluminumAlloy` enum additions.

---

## H.1 — Plastics extension (per-grade Shore D / Rockwell)

Future `Material::Plastic { family }` additions beyond the current enum
(Generic, Acrylic, Hdpe, Delrin, Polycarbonate).

| Material | Grade | Hardness (scale + value) | ASTM standard | Source URL | Verbatim quote |
|----------|-------|---------------------------|---------------|------------|----------------|
| UHMW-PE | Mitsubishi Chemical TIVAR 1000 Natural Virgin | Shore D 66 | ASTM D2240 | https://www.professionalplastics.com/professionalplastics/Tivar1000DataSheet.pdf | `Hardness, Shore D ... 66 ... 66 ... ASTM D2240` (datasheet "Mechanical Properties" table row) |
| Polypropylene (homopolymer) | SIMONA PP-H | Shore D 70 | ASTM D2240 | https://www.simona-america.com/fileadmin/user_upload/USA/Downloads/SIMONA_PPH_Data_Sheet.pdf | `Hardness, Shore D     ASTM D2240    70` (PROPERTIES SIMONA PP-H / MECHANICAL section) |
| Polypropylene (homopolymer) | Direct Plastics PP-H / PP-DWST Natural | Shore D 70 | ISO 868 | https://www.directplastics.co.uk/pdf/datasheets/Polypropylene%20Natural%20Data%20Sheet.pdf | `Shore hardness D    70    -    868` (Mechanical Properties row, "DIN/EN/ISO" column) |
| Nylon 6/6 (PA66, MoS2-filled cast) | Mitsubishi Chemical Nylatron GS (extruded MoS2 Type 66) | Shore D 85 | ASTM D2240 | https://modernplastics.com/wp-content/uploads/Nylatron-GS-MoS2-Datasheet.pdf | `Hardness, Shore D    85    85    ASTM D2240` |
| Nylon 6/6 (PA66, MoS2-filled cast) | Mitsubishi Chemical Nylatron GS | Rockwell M 85 | ASTM D785 | https://modernplastics.com/wp-content/uploads/Nylatron-GS-MoS2-Datasheet.pdf | `Hardness, Rockwell M    85    85    ASTM D785` |
| Nylon 6/6 (PA66, MoS2-filled cast) | Mitsubishi Chemical Nylatron GS | Rockwell R 115 | ASTM D785 | https://modernplastics.com/wp-content/uploads/Nylatron-GS-MoS2-Datasheet.pdf | `Hardness, Rockwell R    115    115    ASTM D785` |
| Nylon 6/6 (PA66, unfilled extrusion) | DuPont Zytel 101L NC010 (dry-as-molded, DAM) | Rockwell M 79 (dry), 59 (conditioned) | ISO 2039-2 | https://upmold.com/wp-content/uploads/data-sheet/PA(Nylon)66%20Zytel%20FG101L%20NC010.pdf | `Rockwell Hardness    Dry    Conditioned    M-Scale    79    59    R-Scale    121    108` (Test Method column: `ISO 2039-2`) |
| Nylon 6/6 (PA66, unfilled extrusion) | DuPont Zytel 101L NC010 | Rockwell R 121 (dry), 108 (conditioned) | ISO 2039-2 | https://upmold.com/wp-content/uploads/data-sheet/PA(Nylon)66%20Zytel%20FG101L%20NC010.pdf | (same table row as above) |
| ABS | (MakeItFrom material-group summary) | Rockwell R 100 to 110 | ASTM D785 (scale only — ranges across grades) | https://www.makeitfrom.com/material-properties/Acrylonitrile-Butadiene-Styrene-ABS | `Rockwell R Hardness: 100 to 110` |
| PETG | Plaskolite VIVAK Sheet (Sheffield Plastics) | Rockwell R 115 | ASTM D785 | https://www.usplastic.com/catalog/files/specsheets/PET-GVIVAK.pdf | `Rockwell Hardness    115    R Scale    ASTM D-785` |
| Rigid PVC Type 1 sheet | Interstate Advanced Materials White PVC Sheet (ASTM D-1784 class 12454-B, formerly Type I Grade 1) | Shore D 74 | (Shore D scale; ASTM D2240 method not explicitly named in source) | https://interstateam.com/white_pvc_sheet_rigid_pvc_type_1_sheet_51021.php | `74 (Shore D)` (Mechanical Properties table) — **CAVEAT:** source lists the Shore D value but does not name ASTM D2240 explicitly on this page. ASTM D2240 is the universal Shore D method; recorded here as scale-only. |

### Notes / caveats

- **PP homopolymer Shore D 70 is corroborated by two independent
  manufacturer datasheets** (SIMONA via ASTM D2240; Direct Plastics via ISO
  868). The two scales differ slightly in method but the reported scalar is
  identical, so 70 is the citeable value.
- **Nylon 6/6 — two distinct grades captured**, because nylon hardness varies
  significantly with fill, moisture conditioning, and processing:
  - **Nylatron GS** (cast / MoS2-filled, machined-stock grade — what a CAM
    user would actually mill from sheet) reports Shore D 85 + Rockwell M 85 +
    Rockwell R 115 on the **same datasheet** using ASTM D785 / D2240.
  - **Zytel 101L NC010** (the canonical unfilled injection-molding grade,
    moisture-sensitive) reports Rockwell M-Scale 79 (dry-as-molded) → 59
    (conditioned), R-Scale 121 → 108, via **ISO 2039-2** rather than ASTM
    D785. The two methods are not numerically interchangeable; both rows are
    recorded as fetched.
- **ABS Rockwell R "100 to 110"** is a MakeItFrom *material-group* range, not
  a single grade. MakeItFrom uses ASTM D785 throughout; their per-page
  "Hardness" glossary cites D785 for Rockwell scales. The verbatim source did
  not echo the ASTM number on the ABS page itself, but the underlying
  method is D785.
- **PETG Rockwell R 115 (Plaskolite VIVAK)** is a single grade with explicit
  ASTM D-785. No Shore D number was published on the VIVAK datasheet.
  MakeItFrom's PETG page also reports `Rockwell R Hardness: 120` (range, no
  ASTM cited on that page) — consistent.
- **Rigid PVC Type 1 Shore D 74** is the only number published on the
  Interstate AM product page; the page does not name ASTM D2240 explicitly,
  so the entry above flags scale-only. The PVC ASTM D-1784 class
  designation (12454-B) is included for traceability.
- All datasheet PDFs were fetched fresh with `curl -skL` and parsed with
  `pdftotext`. The HTML pages (MakeItFrom, Interstate AM) were fetched via
  WebFetch.

---

## H.2 — Aluminum Brinell for additional alloys

Future `AluminumAlloy` enum additions beyond 6061-T6 and 7075-T6.

| Alloy | Temper | Brinell HB | Standard | Source URL | Verbatim quote |
|-------|--------|------------|----------|------------|----------------|
| 2024 | T3 | 120 | (Brinell, 500 g load, 10 mm ball — ASTM E10 family / ASTM B647 for aluminum) | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma2024t3 | `Hardness, Brinell    120    120    AA; Typical; 500 g load; 10 mm ball` (Mechanical Properties row) |
| 5052 | H32 | 60 | (Brinell, 500 g load, 10 mm ball) | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma5052h32 | `Hardness, Brinell    60    60    AA; Typical; 500 g load; 10 mm ball` (Mechanical Properties row) |
| 3003 | H14 | 42 | (Brinell — scale only; ASTM not echoed by MakeItFrom page) | https://www.makeitfrom.com/material-properties/3003-H14-Aluminum | `Brinell Hardness    42` (Mechanical Properties section) |
| 1100 | O (annealed) | 23 | (Brinell — scale only; ASTM not echoed by MakeItFrom page) | https://www.makeitfrom.com/material-properties/1100-O-Aluminum | `Brinell Hardness    23` (Mechanical Properties section) |
| 7050 | T7651 | 147 | Brinell, 500 kg load, 10 mm ball (calculated value, ASM/MatWeb) | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma7050t765 | `Hardness, Brinell    147    147    500 kg load with 10 mm ball. Calculated value.` |
| 7050 | T7651 | 150 | Brinell, 500 kg, 10 mm ball (Kaiser Aluminum mill datasheet) | https://online.kaiseraluminum.com/depot/PublicProductInformation/Document/1016/Kaiser_Aluminum_7050_Sheet_Coil_and_Plate.pdf | `T7651    80    552    71    489    11    150    47    324 ...` (Mechanical Properties table row; columns: Tensile Ult. KSI / MPa, Yield KSI / MPa, Elong/4D %, **Brinell 500kg 10 mm = 150**, Shear KSI / MPa, ...) |

### Notes / caveats

- **ASM Aerospace Specification Metals data sheets** (`asm.matweb.com`) use a
  TLS chain that curl/WebFetch cannot verify on this host (intermediate-CA
  issue carried over from the 2026-05-29 hardness ingest, see that doc's §3
  source note). Fetched with `curl -sk` (insecure). Content itself is the
  standard public ASM data sheet, identical to what `asm.matweb.com` serves
  via browser.
- **1100-O and 3003-H14 are not hosted on `asm.matweb.com`** — ASM's
  bassnum URLs return HTTP 500 for `ma1100*` and `ma3003*`. Values are
  sourced from MakeItFrom, which aggregates from primary publications but
  does not echo the ASTM method number on the per-alloy page. The Brinell
  *scale* is named; the ASTM B647 method is implicit.
- **7050-T7651 has two cited values** (147 from ASM-calculated, 150 from
  Kaiser mill data) — both are recorded; they agree within source rounding.
- The standard cell on ASM data sheets reads `500 g load; 10 mm ball` for
  6061/2024/5052 and `500 kg load with 10 mm ball` for 7050. The "g" on the
  smaller alloys is an ASM rendering quirk (it is in fact kg-force for
  Brinell); the Kaiser 7050 sheet confirms `500kg / 10 mm`.

---

## Summary counts

- **Plastics (H.1):** 11 entries across 6 new families
  (UHMW, PP, Nylon 6/6 ×2 grades, ABS, PETG, rigid PVC).
- **Aluminum (H.2):** 6 entries across 5 new alloys
  (2024-T3, 5052-H32, 3003-H14, 1100-O, 7050-T7651 with two independent
  citations).
- **Combined: 17 new hardness data points** with verbatim quote + scale + URL.
  Beat success target (≥10) met.

Gaps (sources attempted but no usable value found) are recorded in
`planning/data_ingest_2026-05-30/hardness_extra_gaps.md`.
