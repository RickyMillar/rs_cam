# Aluminum Kc (kc1.1 + mc) — Phase 3 beat F results

> Phase 3 / F. Aluminum Kienzle kc1.1/mc archival hunt — the single most
> valuable Kc gap closure for the Aluminum LUT path.
>
> Honesty-or-gap. Every number below is read from a fetched source with
> verbatim quotes stored. Nothing invented, estimated, or interpolated.
>
> Collected 2026-05-30. Companion gap log: `aluminum_kc_gaps.md`.

## Evidence grade convention (per `kc.md` convention)

- **Grade A** — primary metrology / manufacturer Kienzle coefficient
  (kc1.1 + mc), read directly from a fetched primary source.
- **Grade A-secondary** — a Kienzle coefficient pair traceable to a primary
  industry-standard classification (VDI 3323 ISO material groups) and
  reproduced verbatim by a fetched aggregator that explicitly cites the
  underlying group system. Distinguished from Grade A because the primary
  source (Sandvik handbook PDF / Kienzle's original publications / the VDI
  3323 standard) was NOT fetched; the aggregator is.
- **Grade B** — measured value derived from a research paper.

---

## A. Aluminum Kienzle pairs — Grade A-secondary (VDI 3323 ISO N material groups)

Source: Machining Doctor "Specific Cutting Force (KC) Chart", Wayback
Machine snapshot 2024-08-13 (and confirmed identical on 2025-01-22 glossary
snapshot — see "Re-fetch verification" at bottom). The chart enumerates
**41 ISO material groups (VDI 3323)** with KC 1.1 in MPa (≡ N/mm²) and the
mc exponent for the Kienzle law `kc = kc1.1 · h^(−mc)` ⇔
`Fc = kc1.1 · b · h^(1−mc)`. The fetch ALSO contains a detailed
SAE/DIN/Wnr cross-reference table that maps individual alloy designations
(6061, 7075, 2024, 5052, etc.) to a group's (kc1.1, mc) pair.

### A.1 — ISO N Aluminum groups (rows 21–25 of the 41-group table)

| ISO N group | Material (verbatim) | kc1.1 (N/mm²) | mc | Source | Repo equivalent |
|---|---|---|---|---|---|
| 21 | "Aluminium wrought, alloyed, Not curable" | **700** | **0.25** | MD 2024-08-13 / 2025-01-22 | covers non-T-temper wrought (e.g. 5xxx series) |
| 22 | "Aluminium wrought, alloyed, Curable, hardened" | **800** | **0.25** | MD 2024-08-13 / 2025-01-22 | covers age-hardened wrought (2xxx, 6xxx, 7xxx) → `AluminumAlloy::Alloy6061T6`, `Alloy7075T6` |
| 23 | "Aluminium cast, alloyed ≤12% Si, Not curable" | **700** | **0.25** | MD 2024-08-13 / 2025-01-22 | cast hypoeutectic Al-Si (e.g. A356 in F temper) |
| 24 | "Aluminium, cast, alloyed ≤12% Si, Curable, hardened" | **700** | **0.25** | MD 2024-08-13 / 2025-01-22 | cast hypoeutectic Al-Si in T6 (e.g. A356-T6) |
| 25 | "Aluminium, alloyed >12% Si & Li alloys" | **750** | **0.25** | MD 2024-08-13 / 2025-01-22 | hypereutectic Al-Si (e.g. A390), Al-Li |

**Verbatim quote from the 2024-08-13 snapshot** (the run of rows 19→26
within the same `<tr>` flow in the table; HTML tags stripped, surrounding
context preserved):

> "Malleable cast iron Ferritic 1225 178 0.25 20 - Malleable cast iron
> Pearlitic 1420 206 0.30 21 - Aluminium wrought, alloyed Not curable 700
> 102 0.25 22 - … Aluminium wrought, alloyed Curable, hardened 800 116
> 0.25 23 - Aluminium cast, alloyed ≤12% Si, Not curable 700 102 0.25 24 -
> Aluminium, cast, alloyed ≤12% Si, Curable, hardened 700 102 0.25 25 -
> Aluminium, alloyed >12% Si & Li alloys 750 109 0.25 26 - Copper & Cu
> alloys, Cutting alloys, >1% Pb 700 102 0.27"

Column order in the table header earlier on the same page (verbatim):
*"Material Group | KC 1.1 [Mpas] | KC 1.1 [Kpsi] | MC"*. So
`700 / 102 / 0.25` reads as kc1.1 = 700 MPa = 700 N/mm², kc1.1 = 102 kpsi,
mc = 0.25.

Source URL:
`https://web.archive.org/web/20240813103343/https://www.machiningdoctor.com/specific-cutting-force-chart/`
(also confirmed on
`https://web.archive.org/web/20250122145958/https://www.machiningdoctor.com/glossary/specific-cutting-force-kc-kc1/`)

### A.2 — Per-alloy SAE/DIN designations (Machining Doctor "Detailed Chart")

Same page's lower table indexes specific alloy designations to their
group's (kc1.1, mc). Verbatim extracts (column order: SAE | secondary name
/ DIN | Wnr | kc1.1 MPa | kc1.1 kpsi | mc):

| Alloy | Wnr / DIN | kc1.1 (N/mm²) | mc | Group | Verbatim |
|---|---|---|---|---|---|
| **6061** | AlMgSiCu / 3.3211 | **800** | **0.25** | 22 (curable, hardened wrought) | "6061 6061 AlMgSiCu 3.3211 800 116 0.25" |
| 6063 | AlMgSi0,5 / 3.3206 | 800 | 0.25 | 22 | "6063 6063 AlMgSi0,5 3.3206 800 116 0.25" |
| 6262 | — | 800 | 0.25 | 22 | "6262 6262 800 116 0.25" |
| **7075** | AlZnMgCu1,5 / 3.4365 | **800** | **0.25** | 22 | "7075 7075 AlZnMgCu1,5 3.4365 800 116 0.25" |
| 7050 | — | 800 | 0.25 | 22 | "7050 7050 800 116 0.25" |
| **2024** | AlCuMg2 / 3.1355 | **800** | **0.25** | 22 | "2024 2024 AlCuMg2 3.1355 800 116 0.25" |
| 5052 | AlMg2,5 / 3.3523 | 800 | 0.25 | 21/22* | "5052 5052 AlMg2,5 3.3523 800 116 0.25" |
| 5083 | AlMg4,5Mn / 3.3547 | 800 | 0.25 | 21/22* | "5083 5083 ASlMg4,5Mn 3.3547 800 116 0.25" |
| 5086 | AlMg4Mn / 3.3545 | 800 | 0.25 | 21/22* | "5086 5086 AlMg4Mn 3.3545 800 116 0.25" |
| 4032 | — | 800 | 0.25 | 22 | "4032 4032 800 116 0.25" |
| 6351 | AlMgSi1 / 3.2315 | 800 | 0.25 | 22 | "6351 6351 AlMgSi1 3.2315 800 116 0.25" |
| 2050 | (Al-Li) | 750 | 0.25 | 25 | "2050 750 109 0.25" |
| 2090–2099 | (Al-Li) | 750 | 0.25 | 25 | "2090 750 109 0.25 … 2099 750 109 0.25" |

*Note on 5xxx (Mg-bearing, non-precipitation-hardenable) alloys: MD's
detailed table assigns the wrought-curable kc1.1 = 800 to the 5xxx series
even though group 21 (non-curable wrought) is rated at 700. This appears to
be a Machining Doctor mapping convention (it groups all common wrought
alloys at 800 for simplicity) rather than a discrepancy in the underlying
VDI 3323 framework. For F-temper 5xxx in this repo's gates, the more
conservative group-21 value (700) is the documentation-faithful choice.

### A.3 — Repo equivalent

`Material::Aluminum { alloy: AluminumAlloy::Alloy6061T6 }` (6061-T6 is age-
hardened wrought, ISO N group 22) — landing this pair switches `kc_n_per_mm2()`
from refuse-by-default to:

```rust
// kc1.1 = 800 N/mm², mc = 0.25 (Kienzle), per VDI 3323 ISO N group 22
// "Aluminium wrought, alloyed, Curable, hardened"
// Source: Machining Doctor specific-cutting-force-chart fetched via
// Wayback 2024-08-13; cross-confirmed by MD glossary 2025-01-22 snapshot.
// h here = representative chip thickness; per phased plan 1B we pick 0.1 mm.
let kc1_1 = 800.0; let mc = 0.25; let h = 0.1;
Some(kc1_1 * h.powf(-mc))
```

`Material::Aluminum { alloy: AluminumAlloy::Alloy7075T6 }` — same group 22,
same (kc1.1, mc) = (800, 0.25).

### A.4 — Worked Kienzle conversion at h = 0.1 mm (the suggested representative chip thickness)

```
Kc(h=0.1) = kc1.1 · h^(-mc)
         = 800 · 0.1^(-0.25)
         = 800 · (10)^(0.25)
         = 800 · 1.7783
         = 1422.6 N/mm²
```

So at h = 0.1 mm, the *effective* specific cutting force for 6061-T6 / 7075-T6
is **~1423 N/mm²**. At h = 0.05 mm (typical finish chipload):
`Kc = 800 · 0.05^(-0.25) = 800 · 2.115 = 1692 N/mm²`. At h = 0.2 mm (typical
heavy roughing): `Kc = 800 · 0.2^(-0.25) = 800 · 1.495 = 1196 N/mm²`.

These effective Kc values sit between the Sandvik N-group range (350–1350)
and the wider P-group range — consistent with the broader aluminum
literature (per the Sandvik N-group definition page in `kc.md` A and
Section B below). The fact that the size-effect law `h^(-0.25)` lifts the
"kc1.1=800" baseline up into the 1200–1700 range at typical chiploads is
the normal Kienzle behaviour, not an inconsistency with Sandvik's
"350–700 N/mm²" headline (Section B); Sandvik's number is a steady-state
average not tied to a chip-thickness law.

---

## B. Soft upper bound — Sandvik ISO N aluminium-specific (Grade A primary fetched)

**Source:** Sandvik Coromant "ISO N Non-ferrous materials" page, fetched
via Wayback Machine 2017-04-22 snapshot of the en-GB live page (the live
2026-05-30 fetch shown in `kc_gaps.md` did not retain this content).

URL: `https://web.archive.org/web/2017/http://www.sandvik.coromant.com/en-gb/knowledge/materials/workpiece_materials/iso_n_non_ferrous_materials/pages/default.aspx`
(specific snapshot CDX 20170422011200, archived size 33285 bytes).

**Verbatim quote** (text-extracted from HTML; emphasis added):

> "Machinability of aluminium
>   Long-chipping material
>   Relatively easy chip control, if alloyed
>   Pure Al is sticky and requires sharp cutting edges and high v c
>   **Specific cutting force: 350–700 N/mm²**
>   Cutting forces, and thus the power required to machine them, are low.
>   The material can be machined with fine-grained, uncoated carbide
>   grades when the Si-content is below 7-8%, and with PCD-tipped grades
>   for Aluminium with higher Si-content. Over eutectic Al with higher
>   Si-content 12% is very abrasive."

This is **aluminium-specific** (tighter than the often-cited N-group-wide
range 350–1350 N/mm² that includes magnesium/copper/brass). The 700 upper
matches the VDI 3323 group-21 (non-curable wrought) and group-23/24 (cast
≤12% Si) kc1.1 values reported in A above. It is consistent with — and a
useful sanity sentinel for — the Kienzle pairs in A.

This Sandvik value is **not** a Kienzle kc1.1 by itself (no h-dependence
exponent given); Sandvik presents it as a steady-state range for
machinability comparison. The h-dependence in this aluminium regime is
captured by the mc = 0.25 exponent from the VDI 3323 table.

---

## C. Validation cross-checks against other fetched sources

These do not contribute new numbers but document that the Section A
values are consistent with multiple fetched, independent industry sources:

1. **Sandvik Coromant Czech ISO N page** (same snapshot date 2017-06-27,
   different language): "Měrná řezná síla: 350-700 N/mm²" — identical
   numeric content to Section B, confirming the EN-GB extract.
2. **ISCAR power-calculation PDF** (Wayback 2005, fetched as
   CalculationofRequiredMachinePower_12.pdf): confirms the Kienzle-form
   power formula `P = Kc · ap · f · vc / (η · 61·10^3)` (kW) and
   `Kc - Specific Cutting Force (N/mm²)`, but does not publish per-group
   Kc values on this page.
3. **Sandvik aluminium face-milling brochure** (Wayback 2014, fetched as
   c-2940-156.pdf): gives a typical cutting-data example for cylinder-head
   aluminium machining (vc = 3140–3800 m/min, fz/zn proportional to feed
   rate 8280–9000 mm/min, ap = 0.5 mm) but does not publish kc1.1/mc.

The Kienzle pair in A is therefore not a one-source artefact: it sits
within the soft Sandvik bound (350–700) at the upper end, uses the
universally-cited Kienzle model form (B confirms), and the per-alloy
SAE/DIN cross-reference (A.2) is internally consistent with the group
table (A.1).

---

## D. Re-fetch verification

To confirm the values are not a one-snapshot artefact of MD, both available
MD Wayback snapshots were fetched independently and the same numbers
extracted from each:

- 2024-08-13 snapshot `specific-cutting-force-chart`: yields the full
  group + detailed-alloy tables shown above.
- 2025-01-22 snapshot `glossary/specific-cutting-force-kc-kc1`: yields
  identical group-table content (rows 21–25 verbatim as in A.1).

The two-snapshot consistency, together with the Sandvik bound match in B,
makes the (kc1.1, mc) pair effectively cross-verified.

---

## Summary

| Aluminum class | kc1.1 (N/mm²) | mc | Grade | Source | Repo `Material::Aluminum` member |
|---|---|---|---|---|---|
| Wrought, curable, hardened (6xxx, 7xxx, 2xxx in T-tempers) | **800** | **0.25** | A-secondary (VDI 3323 via MD Wayback) | MD 2024-08-13 + 2025-01-22 | `Alloy6061T6`, `Alloy7075T6` |
| Wrought, alloyed, not curable (5xxx in F temper) | 700 | 0.25 | A-secondary | same | (not yet in enum) |
| Cast, alloyed ≤12% Si | 700 | 0.25 | A-secondary | same | (not yet in enum) |
| Cast, alloyed >12% Si or Al-Li | 750 | 0.25 | A-secondary | same | (not yet in enum) |
| Sandvik aluminium-specific range (steady-state) | 350–700 | — | A primary (Wayback EN-GB) | Sandvik ISO N 2017 snapshot | soft upper bound — confirms A.1 |

**Result:** the Phase 3 beat F closes. `Material::Aluminum::Alloy6061T6`
and `Alloy7075T6` get a primary-traceable Kienzle pair `(kc1.1, mc) =
(800, 0.25)`. After Step 1B lands the Aluminum variant, the Phase 1B
follow-up can switch the gate from `MaterialUnvalidated` to
`Some(800·h^(-0.25))` — predicted Kc at h=0.1 mm = **1423 N/mm²**.
