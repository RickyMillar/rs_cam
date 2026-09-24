# G7 fetch notes: material physics (Kc)

Date: 2026-09-24. Agent: G7 research. No cargo run. No file outside
`fetch/G7/` and `scripts/g7_build.py` changed.

Files:

- `sources.json`: 9 manifest-style entries (hash of the downloaded file;
  for Europe PMC it is the JATS XML).
- `candidate_rows.json`: 24 rows, all `x-g7-*`. The build script
  `scripts/g7_build.py` writes both files and stops if a `verbatim` string
  is not in its stored text or a hash does not match.
- `lut_discrepancies.md`: the engine's Kc constants against their documents
  (7 findings, D1–D7).
- `sources/*.txt`: the stored texts. `pdf/`: the downloads (not committed).

## 1. First: the G7 gap is not what PLAN §2 says

PLAN §2 and INVENTORY §1 say "no Kc: power on 208 of 448 shipped cells
only". The matrix (`matrix_2026-09-23.csv`) shows otherwise:

- All four matrix materials have a Kc (`SheetGood::Mdf` 31.4,
  `Plywood::BalticBirch` 13.0, softwood 17.55, hardwood 35.1).
- In `matrix_2026-09-23.csv` I count power on 196 of the 448 shipped
  cells (the PLAN's 208 may be from another matrix run). Power is present
  on every shipped cell of Adaptive, Adaptive3d, Face, Pocket, Rest and
  Zigzag (30 + 30 + 34 + 34 + 34 + 34 = 196) and absent on every finish
  cell (DropCutter, Waterline, Scallop, Profile, Trace, Pencil, ...), for
  all four materials alike. MDF has power on 48 cells, plywood on 36.
  Force (`force_n`) is present on all 448.
- So the cells without power are an **operation-side** gap: the finish
  operations do not route through the power model. A Kc fetch does not
  close it. The reconciler must record this as a correction to PLAN §2.

The Kc fetch still matters, because the numbers behind the 196 powered cells (and the force on all 448) are
weaker than their comments claim (see `lut_discrepancies.md`):

- The force anchor (`force.rs`) is an MDF fit with an unknown chip width,
  attached to hardwood (D1).
- MDF, particleboard and HDPE values are three different quantities (a
  slope, a total kc, a yield stress) used as one (D2–D4).
- Solid wood uses a factor 2.7 fitted as particleboard kc over hardwood shear (D5); plywood omits it (D6).
- The grain factor 2.0 cites a paper that does not measure grain (D7).

## 2. Schema

The LUT observation schema is a chipload row; Kc has no field. Each row
keeps every LUT field name (chipload fields `null`) and adds:
`quantity`, `force_model`, `ks_n_per_mm2` (+ min/max), `int_n_per_mm`
(+ min/max), `kc_n_per_mm2` (+ min/max), `chip_thickness_mm_min/max`,
`cutting_speed_m_s`, `rake_deg`, `helix_deg`, `milling_mode`,
`grain_angle`, `density_kg_m3`, `test_method`, `derived_fields`, plus the
required `extrapolation_group` and `verbatim`.

`quantity` separates the kinds, which must not be mixed:

- `affine_ks_int`: force per mm of edge = Ks·h + Int (Goli 2018).
- `affine_ks_int_figure_read`: same, read by eye from a plot (Goli 2023).
- `density_normalised_ks_int_model`: Curti 2021 Table 5.
- `kc_total`: kc = Fc/(b·h) (Pałubicki).
- `per_tooth_force_line`: N per tooth, chip width not printed (Kopecký).
- `per_blade_force_power_law`: motor-power based (Đurković).
- `kc_textbook_base`: Krsljak Ke1 (grade c).
- `shear_yield_stress_from_cutting`: Yang HDPE (not a cutting force).

Grades: the task reserves `exact` + grade `a` for a printed vendor chart.
No vendor prints a Kc for wood or plastics, so no row is grade a. Printed
values in peer-reviewed papers are `exact` + grade `b`. Figure reads are
`derived` + grade `c`. The textbook coefficient is `exact` + grade `c`.
The reconciler decides the storage shape (a new physics table, not the
chipload LUT, is my proposal).

## 3. What I searched

Reused from the earlier G7 run today (hashes re-verified by a fresh
download; the texts re-converted byte-identical):

- Europe PMC REST `.../PMC6315737/fullTextXML` (Goli 2018),
  `PMC8123317` (Pałubicki 2021), `PMC8747417` (Yang 2022).
- `https://www.woodresearch.sk/wr/201905/12.pdf` (Kopecký 2019),
  `https://www.woodresearch.sk/wr/201702/11.pdf` (Đurković 2017).
  Trap: with the user agent `Mozilla/5.0` the site returns an
  "Access Forbidden" HTML page with HTTP 200. A plain curl gets the PDF.

New this run:

| Query / URL | Result |
|---|---|
| WebSearch "specific cutting force plywood milling N/mm2 chip thickness open access" | only the papers above |
| WebSearch "specific cutting coefficients for the most common engineered wood products" | HAL hal-04274766 |
| `https://hal.science/hal-04274766v1/document` (curl and WebFetch) | Anubis bot wall, "Access Denied" (same as 2026-05-29) |
| HAL API `api.archives-ouvertes.fr/search/?q=halId_s:hal-04274766` | metadata only; file URL is behind Anubis |
| `https://sam.ensam.eu/discover?query=specific+cutting+coefficients+engineered+wood` | **works**: handles 10985/24362 (Goli 2023) and 10985/19997 (Curti 2021) |
| SAM bitstream `LABOMAP_IWMS25th_2023_GOLI.pdf` | stored; plywood Ks only in figures |
| SAM bitstream `LABOMAP_EJWWP_2021_MARCON.pdf` | stored; Table 5 density model printed |
| WebSearch "Wood Handbook FPL specific cutting energy OR cutting power machining chapter" | no machining-energy table |
| `https://www.fpl.fs.usda.gov/documnts/fplgtr/fpl_gtr190.pdf` | HTTP 403 |
| CARB mirror `ww2.arb.ca.gov/.../usfs_wood_handbook_2010.pdf` | stored (hash + excerpt); no machining chapter; 0 hits for specific cutting / cutting energy / cutting power / cutting force |
| WebSearch plywood cutting force bioresources / wood research / drewno | Porankiewicz BioResources papers |
| BioResources `BioRes_02_4_671_681_...LowDenWood.pdf` | stored; multi-factor regressions, no single K; 0 rows |
| BioResources `BioRes_06_4_3687_Porankiewicz_...Pinus` | read; circular-sawing simulation, multi-factor fits, no K at a stated h; not stored |
| WebSearch Aguilera specific cutting energy MDF (Maderas) | scielo 2020 "Cutting energy required ... drying stages" |
| `https://www.scielo.cl/pdf/maderas/v22n4/0718-221X-maderas-00406.pdf` and the `sci_arttext` page (curl and WebFetch) | Cloudflare challenge / HTTP 403 |
| redalyc 48518818002, 48515429001 (Aguilera) | planing roughness and MDF rip-sawing sound; no Kc; not stored |
| redalyc 48550384003 (Orlowski and Ochrymiuk 2017) | circular sawing of Scots pine at MC 35 %, τγ 24.82/18.40 MPa, R 1212.69/34.99 J/m²; sawing, not routing; not stored |
| WebSearch Orlowski fracture toughness plywood MDF | ResearchGate / ScienceDirect only (blocked) |
| WebSearch Eyma 2004; `afs-journal.org/articles/forest/pdf/2004/01/F4106.pdf` | HTTP 403; HAL copy hal-00883828 is behind Anubis |
| WebSearch Naylor 2012; BioResources PDF | read; hand-saw rip tooth, zero rake, HSS, force per mm of depth; not router milling; not stored |
| WebSearch PMMA / polycarbonate specific cutting force | Yan 2020 (PMC7796128) read: temperature and chips only, no force; not stored |
| WebSearch kc1.1 aluminium 6061 | Machining Doctor `mds/?matId=3850` HTTP 403; Scribd mirrors not fetched |

Earlier campaigns already proved these dead ends; I did not repeat them:
ScienceDirect, ResearchGate, Machining Doctor (403), Trifunović 2021 and
Chabbi 2017 POM (paywall), Korkmaz 2017 PMMA (403), polycarbonate (no
primary study). See `planning/data_ingest_2026-05-29/kc_gaps.md` and
`planning/data_ingest_2026-05-30/kc_extra_gaps.md`.

## 4. What I found (per target)

| Target | Found | Rows |
|---|---|---|
| MDF | Goli 2018 Ks 31.44 / Int 3.36 (printed); Kopecký 2019 per-tooth line (width not printed); Goli 2023 figure | 1 + 2 + 1 |
| Particleboard | Pałubicki 2021 kc 32.0 / 37.6 (printed, total kc); Goli 2023 figure | 2 + 1 |
| Plywood | Goli 2023 poplar plywood, **figure only**; beech LVL Table 4 (a veneer product, not plywood) | 4 (derived) + 2 |
| Softwood / hardwood | Curti 2021 Table 5 density model, 287–1080 kg/m³ (printed); Đurković 2017 oak (printed power law); Krsljak Ke1 (grade c) | 6 + 2 |
| Acrylic (PMMA) | nothing printed that I could reach | 0 |
| Polycarbonate | nothing printed (no primary study exists, as in 2026-05-29) | 0 |
| HDPE | Yang 2022 yield stress only (not a Kc) | 2 |
| PTFE (not a target; no engine family) | Goli 2018 Ks 20.33 / Int 2.71 | 1 |
| Aluminium 6061 | nothing new reachable | 0 |

The chip thickness each value applies to is in each row. All router-type
measurements cover h 0.04–0.10 mm (Goli, Curti) or up to 0.12 mm
(Kopecký); Pałubicki up to 0.31 mm; Đurković 0.02–0.13 mm.

## 5. What I could not find (refusal reasons)

- **Plywood, printed:** no document I could reach prints a plywood Ks or kc
  as a number. The one measurement (Goli 2023) prints plots only; the HAL
  full text is the same paper. Baltic birch and softwood plywood: no data
  at all.
- **Acrylic, polycarbonate, POM, and the other plastics:** not published
  anywhere I could reach. HDPE has a yield stress, not a cutting force.
- **USDA Wood Handbook:** verified this session on the 2010 edition (CARB
  mirror; the FPL original returns 403): no machining chapter, and no line
  with "specific cutting", "cutting energy", "cutting power" or "cutting
  force". "Machinability" is a qualitative species rating only.
  Koch "Wood Machining Processes" (1964) and Kivimaa (1950) are books; not
  reachable; no number recorded.
- **A shear-strength-to-milling-Kc factor** (the engine's 2.7): not printed
  anywhere found.
- **A Janka-to-Kc law** (the engine's `janka/100`): not printed anywhere
  found. Curti 2021 prints a density law instead.
- **Aluminium 6061 kc1.1 / mc from a printed chart:** blocked (403).

## 6. Hints for the reconciler (not rulings)

- One anchor, one quantity. Goli 2018 (MDF) and Curti 2021 (solid wood)
  use the same method and print force per mm of edge, the quantity that
  `force.rs` needs. Together they cover MDF, PTFE, beech LVL and five solid
  woods at 287–1080 kg/m³, h 0.04–0.10 mm, with a 20 or 80 mm tool at
  about 3–13 m/s.
- Limits a claim must carry: h 0.04–0.10 mm (the size effect below
  0.04 mm is outside every router study here), density 287–1080 kg/m³,
  rake 25°, NRMSE 8–38 % (Curti Table 6).
- Plywood: a derived claim from Curti's density law, confirmed against the
  Goli 2023 figure read, is possible, but it has one weak witness. Refuse
  or ship as one-witness, by ruling.
- Plastics other than PTFE: refuse (no printed Kc).
