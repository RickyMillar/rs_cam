# Plastics Kc — Round-2 archival hunt (Phase 3 / Beat E)

> Staging only. Touches NO live code. Every numeric Kc-equiv value below is
> read from a fetched primary source with a verbatim quote stored alongside.
> No value is invented, estimated, or interpolated. Back-calculations show the
> arithmetic explicitly.
>
> Collected 2026-05-30 as the Round-2 follow-on to
> `planning/data_ingest_2026-05-29/kc.md`. Targets the three polymer gaps
> (POM/Delrin, PMMA, Polycarbonate) that the Round-1 ingest could not close.
>
> **Round-2 net new:** one Grade-B back-calculated PMMA cutting-force-yield
> value from a freshly-discovered fetched primary source (PMC12473824, 2025).
> POM/Delrin and Polycarbonate remain GAP — see `kc_extra_gaps.md` for the
> exhaustive list of attempts. The new PMMA value is **NOT** directly usable
> as a milling Kc; it is recorded with the explicit caveat that the size
> effect at the source's nanoscale chip thickness dramatically inflates the
> apparent Kc relative to milling at typical chip thicknesses (≥10 µm). See
> caveat block.

## Evidence grade convention (unchanged from Round-1)

- **Grade A** — primary metrology / manufacturer Kienzle coefficient.
- **Grade B** — measured value back-calculated from a published force or read
  as specific cutting energy. Arithmetic shown.

---

## C. PLASTICS — Round-2 net new

| Material | Kc-equiv (N/mm²) | Conditions | Grade | Source | Verbatim quote | Repo equivalent |
|---|---|---|---|---|---|---|
| **PMMA / Acrylic** (nanoscale orthogonal cutting; SIZE-EFFECT INFLATED — not directly milling-applicable) | **276.5** (back-calculated, experimental, 200 nm chip thickness) | Plain PMMA, ultramicrotome orthogonal cutting, single-crystal diamond knife (PCB 209A12 piezo dyno), 45° rake, cutting speed 1.0 mm/s, depth of cut h = 200 nm = 0.0002 mm, force reported per unit chip width | B | Liu, Yan, Tang, Ji, Zhao 2025, "Finite Element Analysis and Experimental Investigation on the Machinability of PMMA/CNT Composites via Nanosectioning", *Polymers* 17(18):2441, DOI 10.3390/polym17182441; fetched via EuropePMC PMC12473824 (CC BY 4.0; open access) | "for a cutting thickness of 200 nm, the simulated resultant cutting force for plain PMMA stands at about 30.4 N/m, while the experimental value is approximately 55.3 N/m" | **`Material::Plastic { family: Acrylic }`** — but per Round-2 hard rule, **do NOT** promote this as the plastic-family Kc value; see caveat below. |

### Arithmetic — back-calculation of PMMA Kc-equiv (Liu et al. 2025)

Force reported per unit chip width (b): Fc/b = 55.3 N/m = 0.0553 N/mm
Chip thickness: h = 200 nm = 2 × 10⁻⁴ mm
Specific cutting force: Kc = (Fc/b) / h = 0.0553 N/mm ÷ 0.0002 mm = **276.5 N/mm²**

Cross-check at simulated value (for sanity): 30.4 N/m ÷ 0.0002 mm = 152 N/mm².
The experimental value runs ~1.8× higher than the FEM-simulated value (the
paper attributes this to strain-gradient ploughing not captured in the
Mulliken–Boyce model — i.e. size effect).

### SIZE-EFFECT CAVEAT — why this is recorded but NOT promotable as a milling Kc

The 276.5 N/mm² value comes from **nanoscale orthogonal cutting** at h = 200
nm with a 45° (very positive) diamond rake. At such small chip thickness the
edge-radius / ploughing component dominates over the bulk shearing component,
inflating apparent specific cutting force by an order of magnitude versus
the same material at typical milling chip thickness (~10 µm and up). The
paper itself acknowledges this discrepancy:

> "the model also disregards strain gradient effects, which are particularly
> important in nanosectioning operations. At small cutting depths, such as
> those used in this study (60[–200 nm]), [...] These gradients result in
> stress concentrations that significantly affect the cutting force but are
> not captured by the current model." (PMC12473824, §5.1)

Applying 276.5 N/mm² to a milling chip-thickness regime (e.g. 30–200 µm,
which is 150–1000× larger) would severely **over-predict** cutting force —
the inverse of the wood-Kc problem that Phase 2 of the ingest plan is
addressing. The size-effect ratio for polymers at 1 mm chip thickness
extrapolating from 200 nm is roughly (200 nm / 1 mm)^(−mc) ≈ (1/5000)^(−0.2)
≈ 5.5× to 10×, suggesting a milling-regime PMMA Kc closer to **30–50 N/mm²**
(in line with HDPE's 33.85–46.89 from Round-1 Yang 2022). But that
extrapolation is **not a fetched primary measurement** and is therefore NOT
recorded here as a Kc value.

**Disposition.** Promote ONLY as a "fetched primary nanoscale PMMA cutting
yield reference"; **do NOT** wire this into `Material::Plastic { family:
Acrylic }::kc_n_per_mm2()` as the milling Kc. The architecturally-correct
Phase 1A pattern still applies: `Acrylic.kc_n_per_mm2() = None` — refuse the
gate with `MaterialUnvalidated` until a fetched milling-regime PMMA Kc lands.

This entry exists so a future review can (a) avoid re-finding the same
nanoscale source thinking it's new, and (b) use it as a sanity-check upper
bound on any future PMMA milling Kc claim.

### POM / Delrin — Round-2 confirms paywalled

Re-attempted Trifunović 2021 (J. Cleaner Production 303:127043) and Chabbi
2017 (Measurement 95:99–115, DOI 10.1016/j.measurement.2017.10.067) via
Wayback Machine CDX search, Memento Time Travel API, and Unpaywall:

- Wayback CDX for `sciencedirect.com/science/article/pii/S0959652621012622`
  (Trifunović): only one snapshot in 2025 returning HTTP 302 (redirect) and
  one returning HTTP 403 — **no successful full-text capture exists**.
- Unpaywall API: `is_oa: False` for both DOIs. No repository / preprint / OA
  mirror is registered.
- EuropePMC search for `polyoxymethylene cutting force` + `POM-C specific
  cutting`: returns 215 hits, but the closest on-target hit (PMC9864973 —
  *Determination of Processing Precision of Hole in Industrial Plastic
  Materials*, includes POM-C) measures **hole-diameter precision** rather
  than cutting force; no Fc/SCE data extractable.

**Disposition unchanged from Round-1.** POM/Delrin Kc remains GAP. No primary
fetched force or SCE measurement obtained. See `kc_extra_gaps.md` §1 for the
attempt log.

### Polycarbonate (PC) — Round-2 confirms NO PRIMARY SOURCE EXISTS

Searched Google Scholar (`polycarbonate cutting force specific kc milling`),
EuropePMC (`polycarbonate cutting force milling`, 26 PMC hits) and PMC
(`polycarbonate specific cutting`, 21 hits) — every relevant-looking hit is
either:

- a different material (vitrimers, PDMS, fiber-reinforced composites), or
- an ultraprecision review (no per-material Kc data), or
- a 3D-printed part / drilling-precision paper without force measurement.

No manufacturer datasheet (Bayer/Covestro Makrolon, SABIC Lexan) publishes a
recommended cutting-force coefficient — datasheets list mechanical properties
(tensile, flexural, Izod) and tooling recommendations but not Kc.

**Disposition unchanged from Round-1.** PC Kc remains the architecturally
correct "no primary source; must refuse with `MaterialUnvalidated`" state.
This is itself a valuable, defensible finding per the Phase 1A.0 design.

---

## SUMMARY (Round-2 contribution)

- **PMMA**: 1 new Grade-B back-calculated value (276.5 N/mm²) recorded with
  size-effect caveat — not promotable as milling Kc but logged as fetched
  nanoscale upper-bound reference. Source: Liu et al. 2025, *Polymers*
  17(18):2441 (PMC12473824, CC BY 4.0).
- **POM/Delrin**: 0 new values. Paywalled sources re-confirmed via Unpaywall
  + Wayback CDX; alternative PMC sources (e.g. PMC9864973) do not measure
  force.
- **Polycarbonate**: 0 new values. Comprehensive PMC + EuropePMC + manufacturer
  datasheet search re-confirms the "no primary source exists" finding from
  Round-1.

Net effect on Phase 1A architecture: **no change**. The plan to set
PC/Acrylic/Delrin/Generic to `kc_n_per_mm2() -> None` remains correct. The
PMMA Round-2 fetch is recorded as a reference for future bounding work, not
as a usable milling Kc.
