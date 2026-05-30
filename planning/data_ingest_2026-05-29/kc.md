# Specific Cutting Force (Kc) — Verified Citeable Values

> Staging data only. Touches NO live code. SAFETY-CRITICAL: every numeric Kc
> below is read from a fetched source with a verbatim quote stored alongside.
> No value here is invented, estimated, or interpolated. Back-calculated /
> SCE-read values are Grade B and show the arithmetic + source figure.
>
> Collected 2026-05-29. Sources fetched live; paywalled / firewalled sources
> are in `kc_gaps.md`.

## Evidence grade convention

- **Grade A** — primary metrology / manufacturer Kienzle coefficient
  (kc1.1 + mc), read directly from a fetched primary source.
- **Grade B** — measured value derived from a research paper: back-calculated
  from a published force, or read as specific cutting energy (J/mm³ = N/mm²),
  or a measured specific cutting coefficient Ks from an orthogonal/round-shape
  test. Arithmetic and source figure shown.

## Units note

1 J/mm³ = 1 N·mm / mm³ = 1 N/mm² = 1 MPa. Specific cutting energy (SCE) and
specific cutting force are dimensionally identical; cutting "yield stress" and
"specific cutting pressure Ks" reported in polymer/wood papers are in the same
N/mm² family and are recorded as such with the method noted.

---

## A. METALS (Grade A — Kienzle kc1.1 + mc)

No live Kc table exists in `material.rs` for metals (the repo models wood,
sheet goods, plastics, foam only). These are recorded for completeness / future
metal support and to validate the Kienzle model the deflection/power gates use.

| Material | ISO grp | kc1.1 (N/mm²) | mc | Conditions | Grade | Source | Verbatim quote | Repo value |
|---|---|---|---|---|---|---|---|---|
| C15 steel (carbon steel) | P | **1639.05** | mc = 0.25 (1−mv = 0.75) | Drilling, Ø10 mm twist drill, 22.3 m/min, f 0.056–0.179 mm/rev, Kistler dyno, graphical extrapolation to h=1 mm | A | Sekulić et al. 2014, "Prediction of the Main Cutting Force in Drilling by Kienzle Equation", TMT 2014, tmt.unze.ba/zbornik/TMT2014/TMT2014_003.pdf, p.8 | "The results for Kienzle constants were kv1.1=1639,05 N/mm2 and 1-mv=0,75." | — (no metal in repo) |
| AISI H13 / 1.2344 annealed hot-work steel | P | **2450** | mc = 0.23 | Averaged values, neutral rake reference | A | ISCAR "Machining Calculations" technical article (iscar.com/en-hq/technical-articles/year-2025/machining-calculations) | "kc1 =2450 N/mm2 (355 ksi) and mc=0.23" | — |

### Aluminum / non-ferrous (ISO N) — range only, NOT a single citeable point

- Sandvik handbook range cited in the acquisition doc: **N group 350–1350 N/mm²**.
  The live Sandvik "Specific cutting force" knowledge page
  (sandvik.coromant.com/en-us/knowledge/materials/specific-cutting-force) was
  fetched and CONFIRMS the definition + model but the numeric kc1.1/mc table has
  been removed from the live site. Verbatim from that page: *"the force, Fc, in
  the cutting direction needed to cut a chip area of 1 mm that has a thickness of
  1 mm"* and the model *"kc = kc1 · h^(−mc)"*; it also states only *"The kc1
  value is different for the six material groups, and also varies within each
  group"* — **no numbers**.
- Search corroboration (not a fetched primary table, so NOT recorded as a value):
  multiple aggregators state aluminum kc ≈ 350–700 N/mm² depending on alloy.
- **GAP**: a fetched primary kc1.1 + mc *pair for an aluminum alloy* was not
  obtained — Machining Doctor (the doc's named aggregator), the Sandvik
  Technical Guide Scribd mirror, and the ResearchGate kc1/mc figure all returned
  403/Cloudflare. Logged in `kc_gaps.md`. Do NOT fabricate an N-group number.

**Kienzle model (confirmed across all metal sources):**
`Fc = kc1.1 · b · h^(1−mc)` ⇔ `kc = kc1.1 · h^(−mc)` where h = uncut chip
thickness (mm), b = chip width (mm), mc ≈ 0.2–0.3 typical.

---

## B. WOOD / ENGINEERED WOOD PRODUCTS (Grade B — measured)

| Material | Kc (N/mm²) | Conditions | Grade | Source | Verbatim quote | Repo kc_n_per_mm2() |
|---|---|---|---|---|---|---|
| **Particleboard** (slow milling) | **32.0** | Peripheral up-milling, vc=40 m/s, rake 13°, h up to ~0.31 mm | B | Pałubicki 2021, *Materials* 14(9):2208, DOI 10.3390/ma14092208 (open access; fetched via PMC8123317) | "The obtained average specific principal cutting forces for particleboard peripheral up-milling are equal to 32.0 N/mm2 for slow and 37.6 N/mm2 for fast milling." | **9.0** (SheetGood::Particleboard) |
| **Particleboard** (fast milling) | **37.6** | Peripheral up-milling, vc=60 m/s, rake 13° | B | same as above | "...32.0 N/mm2 for slow and 37.6 N/mm2 for fast milling." Also: "60 m/s speed increased the average value of 17.5% compared to 40 m/s" | **9.0** |
| **MDF** | **31.44** (SD 2.68; range 25.81–35.58) | Round-shape (rotational) machining, specific cutting coefficient Ks; isotropic in plane | B | "Specific Cutting Forces of Isotropic and Orthotropic Engineered Wood Products by Round Shape Machining", PMC6315737 (fetched) | "MDF: Average (SD) Ks [N mm−2] 31.44 (2.68)" | **10.0** (SheetGood::Mdf) |
| **PTFE** (bonus, isotropic polymer reference) | **20.33** (SD 2.90; range 14.46–24.81) | Round-shape machining Ks | B | PMC6315737 (fetched) | "PTFE: Average (SD) Ks [N mm−2] 20.33 (2.90)" | — |
| **Beech LVL** (orthotropic) | 6.61–51.04 (orientation-dependent; min @100°, max @60°) | Round-shape machining Ks across grain angles | B | PMC6315737 (fetched) | "Beech LVL: Ks values ranged from 6.61 to 51.04 N/mm² depending on grain orientation" | — |
| **Poplar LVL** (orthotropic) | up to 38.98 (max @70°; some negative artifacts) | Round-shape machining Ks across grain angles | B | PMC6315737 (fetched) | "Poplar LVL: Ks values ranged from -12.41 to 38.98 N/mm² across different grain orientations" | — |

### Pałubicki chip-thickness behavior (verbatim)
> "the kc may be considered constant" across the range studied (with "delicate
> fluctuations from mean values"); the specific cutting *thrust* force
> "decreases linearly" with increasing instantaneous uncut chip thickness.

So for particleboard the principal Kc is roughly chip-thickness-independent in
the milling regime — unlike the Kienzle h^(−mc) law for metals.

### FPL shear-parallel-to-grain (Grade B *reference property*, NOT Kc directly)

Shear strength parallel to grain is a material property used to **derive /
sanity-check** per-species solid-wood Kc. It is NOT the same quantity as Kc and
is recorded only as a cross-reference. From FPL Wood Handbook GTR-190 Ch.5,
Table 5-3a (metric, clear specimens at 12% MC), fetched as PDF from
precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf:

| Species (repo) | Shear ∥ grain (kPa → MPa) | Repo kc_n_per_mm2() | Note |
|---|---|---|---|
| Sugar maple (= Hard Maple) | 16,100 → **16.1** | 15.0 | repo close to bare shear strength |
| Black walnut (= Walnut) | 9,400 → **9.4** | 12.0 | repo > shear strength |
| White oak | 13,800 → **13.8** | 14.0 | repo ≈ shear strength |
| Eastern white pine (softwood) | 6,200 → **6.2** | 6.0 (GenericSoftwood) | repo ≈ shear strength |
| Loblolly pine (S. yellow pine class) | 9,600 → **9.6** | 7.0 (SouthernYellowPine) | repo < shear strength |
| Longleaf pine (S. yellow pine class) | 10,400 → **10.4** | 7.0 | — |

Verbatim source values (Table 5-3a / 5-3b, 12% MC, "Shear parallel to grain"
column): Sugar maple 16,100 kPa; Black (sugar-class) walnut 9,400 kPa; White
oak 13,800 kPa; Eastern white pine 6,200 kPa; Loblolly pine 9,600 kPa; Longleaf
pine 10,400 kPa. (Birch is on Table 5-3 hardwood pages not captured here — GAP.)

---

## C. PLASTICS (Grade B — derived / measured cutting stress)

| Material | Kc-equiv (N/mm²) | Conditions | Grade | Source | Verbatim quote | Repo kc_n_per_mm2() |
|---|---|---|---|---|---|---|
| **HDPE** (15° rake) | **46.89** (cutting yield stress) | Orthogonal cutting, carbide tool, clearance 7°, cut depths 60/120/180 µm | B | Yang et al. 2022, *Polymers* 14(1):189, DOI 10.3390/polym14010189 (open access; fetched via PMC8747417) | "The yielding stress derived from the analyses have the values of 46.89 and 33.85 MPa for the two cutting tools" (15° and 30° rake) | **4.0** (all Plastic) |
| **HDPE** (30° rake) | **33.85** (cutting yield stress) | same; 30° rake tool | B | same | same quote; quasi-static baseline "27.4 MPa" | **4.0** |

**HDPE interpretation:** these are the *cutting yield stress* (effective flow
stress that sets the cutting-force scale in the FEM), the closest published
analog to Kc for HDPE. They are NOT a Kienzle kc1.1. Rake-angle dependent:
sharper effective rake (30°) → lower stress (33.85) than 15° (46.89), both
elevated above the 27.4 MPa quasi-static compression value. Use 33.85–46.89
N/mm² as the HDPE Kc band, Grade B.

### Acrylic (PMMA) — force study located but NO clean Kc back-calculation
- **Korkmaz et al. 2017**, "Micromilling of PMMA Using Single-Crystal Diamond
  Tools", Procedia Manufacturing 10:683–693, DOI 10.1016/j.promfg.2017.07.017
  (open access). Reports *process forces* (Ø450 µm SCD endmill; spindle 90/120/
  150 krpm; feed 5/10/15 µm/flute; ap 50/100 µm). The ScienceDirect HTML
  (S235197891730197X) returned 403 on fetch, so the specific Fc value tied to a
  known chip area needed for Kc ≈ Fc/(ap·f) was NOT obtained from a fetched
  source. **Recorded as a GAP, not a value** — see `kc_gaps.md`. Do NOT
  fabricate a PMMA Kc. (Acrylic currently uses the generic Plastic Kc=4.0 in
  the repo.)

### POM / Delrin — paywalled (GAP)
Trifunović 2021 (J. Cleaner Production, POM-C SCE) and Chabbi 2017 (Measurement;
IJAMT, POM-C force regressions) are paywalled — see `kc_gaps.md`. No fetched
primary number → no Delrin/POM Kc recorded.

### Polycarbonate (PC) — NO PRIMARY SOURCE
There is **no primary machining-force study** for polycarbonate. Per the
overriding rule, PC's Kc is "no primary source; must be datasheet-estimated" —
**no number recorded here.** See `kc_gaps.md`. (Repo uses generic Plastic
Kc=4.0 for Polycarbonate.)

---

## KEY DISCREPANCIES vs repo `material.rs::kc_n_per_mm2()`

The single largest, safety-relevant finding: **the repo's wood/EWP Kc constants
are ~3–4× LOWER than the measured literature.**

| Material | Repo Kc | Literature Kc (measured, Grade B) | Ratio (lit ÷ repo) |
|---|---|---|---|
| Particleboard | 9.0 | 32.0 (slow) – 37.6 (fast) | **3.6× – 4.2× too low** |
| MDF | 10.0 | 31.44 | **3.1× too low** |
| HDPE | 4.0 (generic plastic) | 33.85 – 46.89 (cutting yield stress) | **8.5× – 11.7× too low** |
| Hard Maple (solid) | 15.0 | (shear-∥ 16.1 reference; no direct Kc) | repo ≈ shear strength |
| Walnut (solid) | 12.0 | (shear-∥ 9.4 reference) | repo above shear strength |

Caveats on the discrepancy (do NOT "fix" anything — flag only):
- The repo wood Kc constants track *shear-parallel-to-grain strength* (≈6–16
  MPa) far better than they track *milling specific cutting force* (≈30–40
  N/mm² for boards). Two different physical quantities.
- Measured EWP Kc (Pałubicki 32–37.6; round-shape MDF 31.4) is for *peripheral
  / rotational milling* at low chip thickness, where edge-radius ploughing
  inflates the apparent specific force well above the bulk shear strength. This
  is the expected size-effect — it is real, not an error, but means the repo's
  shear-strength-derived numbers under-predict milling force by 3–4×.
- HDPE is the worst case: repo lumps ALL plastics at 4.0 N/mm² while measured
  HDPE cutting stress is 34–47 N/mm². A wrong-by-10× Kc directly mis-scales the
  power and deflection gates for plastic jobs.

---

## SUMMARY COUNT

- **Metals (Grade A):** 2 verified kc1.1+mc pairs (C15 steel 1639.05 / mc 0.25;
  H13 steel 2450 / mc 0.23). Aluminum N-group: range 350–1350 confirmed by
  Sandvik definition page but NO fetched numeric pair → GAP.
- **Wood / EWP (Grade B):** 5 verified values (particleboard 32.0 & 37.6; MDF
  31.44; PTFE 20.33; + beech/poplar LVL orientation ranges) plus 6 FPL
  shear-∥ reference properties for per-species derivation.
- **Plastics (Grade B):** 2 verified (HDPE 33.85 & 46.89 cutting yield stress).
  PMMA force study located but Kc not back-calculable from a fetched figure
  (GAP); POM/Delrin paywalled (GAP); PC has no primary source (explicit GAP).
