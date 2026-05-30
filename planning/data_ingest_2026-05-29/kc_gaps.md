# Kc Acquisition Gaps — paywalled / unreachable / nonexistent sources

> Staging only. Every entry here is a Kc value that could NOT be obtained from a
> fetched primary source on 2026-05-29. Per the overriding rule, NO number is
> recorded for any of these — they are logged so a later session (or one with
> library/paywall access) can close them. Collected 2026-05-29.

---

## 1. Aluminum / non-ferrous (ISO N) kc1.1 + mc pair — UNREACHABLE

- **What's missing:** a fetched primary kc1.1 + mc *pair for a named aluminum
  alloy* (e.g. 6061, AlMgSi, AlSi cast). The Sandvik handbook range cited in the
  acquisition doc is **N group 350–1350 N/mm²**, but the live Sandvik knowledge
  page no longer hosts the numeric table (confirmed by fetch — only the
  definition and the `kc = kc1·h^(−mc)` model survive on-page).
- **Sources attempted (all blocked):**
  - https://www.machiningdoctor.com/glossary/specific-cutting-force-kc-kc1/ — HTTP 403 (Cloudflare). This is the doc's named aggregator that "reproduces kc1.1 + mc for 41 material groups."
  - https://www.machiningdoctor.com/calculators/machining-power/ — HTTP 403.
  - https://www.scribd.com/document/682202846/... (Machining Doctor mirror) — Scribd body not served to fetcher (metadata only).
  - https://www.researchgate.net/figure/...kc1-and-mc-parameters...P-M... — HTTP 403.
  - https://www.sciencedirect.com/topics/engineering/specific-cutting-force — HTTP 403.
  - web.archive.org — fetcher explicitly blocked from web.archive.org.
- **Reason:** Cloudflare/anti-bot 403 on the aggregators; Sandvik removed the
  table from the live site (per acquisition-doc caveat, confirmed).
- **How to close:** read the Sandvik *Metal Cutting Technology Training
  Handbook* PDF or *Technical Guide – Materials ISO* PDF (mirrored on
  pdfcoffee / ResearchGate per the acquisition doc), or open Machining Doctor in
  a real browser. Until then, only the 350–1350 N/mm² *range* is citeable, not a
  point value.

## 2. PMMA / Acrylic — force study found, Kc NOT back-calculable from fetch

- **Source:** Korkmaz, Önler & Özdoğanlar 2017, "Micromilling of PMMA Using
  Single-Crystal Diamond Tools," Procedia Manufacturing 10:683–693,
  DOI 10.1016/j.promfg.2017.07.017 (open access in principle).
- **What's missing:** a specific cutting force value (Fc) tied to a known chip
  area (ap·f) so Kc ≈ Fc/(ap·f) could be computed. The paper reports process
  forces parametrically (feed 5/10/15 µm/flute; ap 50/100 µm; spindle
  90/120/150 krpm) but the figure with the force magnitudes was not served.
- **Attempted:** https://www.sciencedirect.com/science/article/pii/S235197891730197X — HTTP 403.
- **Reason:** ScienceDirect anti-bot 403; ResearchGate PDF also 403-prone.
- **How to close:** fetch the open-access Procedia PDF (Elsevier OA) directly or
  via institutional access, read the cutting-force figure, then back-calculate.

## 3. POM-C / Delrin SCE — PAYWALLED

- **Trifunović et al. 2021**, "Investigation of cutting and specific cutting
  energy in turning of POM-C using a PCD tool," J. Cleaner Production
  303:127043, DOI 10.1016/j.jclepro.2021.127043.
  - The single most on-target polymer SCE study (produces J/mm³ = N/mm² maps for
    POM-C). **Paywalled** (abstract free, full text behind Elsevier paywall).
- **Chabbi et al. 2017**, Measurement 95:99–115
  (DOI 10.1016/j.measurement.2016.09.043) and Int. J. Adv. Manuf. Technol.
  91:2267–2290 (DOI 10.1007/s00170-016-9858-8): POM-C tangential force Fz
  regression models (would allow Kc = Fc/(ap·f) back-calc). **Paywalled.**
- **Reason:** publisher paywall (Elsevier / Springer); abstracts only.
- **How to close:** institutional / library access to either DOI; read the SCE
  map (Trifunović) or the Fz regression (Chabbi) and back-calculate.

## 4. Polycarbonate (PC) — NO PRIMARY SOURCE EXISTS

- **Status:** per the task brief and the acquisition doc, there is **no primary
  machining-force study for polycarbonate**. This is not a fetch failure — the
  data does not exist in the literature.
- **Recorded explicitly as:** "no primary source; must be datasheet-estimated."
  NO Kc number is recorded for PC anywhere in `kc.md`.
- **How to close:** would require either a future PC orthogonal-cutting / SCE
  study, or an explicit datasheet-based *estimate* clearly labelled as
  estimated (NOT measured) — out of scope for this measured-data pass.

## 5. HAL EWP coefficients paper — ANUBIS-BLOCKED

- **Source:** "Specific cutting coefficients for the most common engineered wood
  products," HAL hal-04274766 (would give particleboard/MDF/OSB/plywood Kc with
  intercepts + anisotropy).
- **Attempted:**
  - https://hal.science/hal-04274766 — Anubis anti-bot "Access Denied".
  - https://hal.science/hal-04274766v1/document — Anubis "Access Denied".
- **Reason:** HAL deploys the Anubis proof-of-work bot wall; the fetcher cannot
  pass it.
- **Mitigation:** the same EWP-coefficient data class is partially covered by
  the *fetched* PMC6315737 round-shape paper (MDF 31.44, PTFE 20.33, beech/
  poplar LVL ranges) and Pałubicki 2021 (particleboard 32.0/37.6), so the wood
  side is not a hard gap — but the OSB and plywood point values from HAL remain
  unobtained.
- **How to close:** fetch the HAL PDF via a browser able to clear Anubis, or
  find a mirror (the acquisition doc notes search snippets reproduce the
  isotropy finding but not the numeric coefficient table).

## 6. MDPI rate-limiting (transient, NOT a hard gap)

- mdpi.com returned HTTP 403 on direct article fetches (ma14092208,
  polym14010189) after an initial success — transient rate-limit. Both papers
  were successfully read via their **PMC mirrors** (PMC8123317, PMC8747417), so
  the values ARE recorded in `kc.md`. Logged only so a re-run knows to prefer
  the PMC mirror over the MDPI canonical URL.
