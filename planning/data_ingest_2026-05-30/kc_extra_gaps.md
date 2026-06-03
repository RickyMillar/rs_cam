# Plastics Kc — Round-2 acquisition gaps

> Round-2 follow-on to `planning/data_ingest_2026-05-29/kc_gaps.md`.
> Collected 2026-05-30. Every entry here is a Kc target that could NOT be
> obtained from a fetched primary source. Per the overriding rule, NO number
> is recorded for any of these.

---

## 1. POM / Delrin — paywall + Wayback redirect-only confirmed

### 1A. Trifunović 2021 (J. Cleaner Production 303:127043)

**DOI:** `10.1016/j.jclepro.2021.127043`
**Title:** *"Investigation of cutting and specific cutting energy in turning
of POM-C using a PCD tool: Analysis and some optimization aspects"*
**ScienceDirect PII:** S0959652621012622

**Round-2 attempts (all blocked):**

1. Wayback DOI fetch:
   `curl https://web.archive.org/web/2024/https://doi.org/10.1016/j.jclepro.2021.127043`
   → HTTP 200, **but body is the Elsevier "Linking Hub" redirect intermediate page**
   (`<meta REFRESH>` to the ScienceDirect URL), **NOT** the article body.
   Verbatim title in the redirect page confirms identity:
   > "articleName : 'Investigation of cutting and specific cutting energy in
   > turning of POM-C using a PCD tool: Analysis and some optimization
   > aspects'"

2. Wayback CDX search on the canonical PII URL:
   `curl 'https://web.archive.org/cdx/search/cdx?url=sciencedirect.com/science/article/pii/S0959652621012622&output=json&limit=20'`
   → Returns only 2 snapshots:
   - `20250502234019` — HTTP 302 (redirect)
   - `20250830202415` — HTTP 403 (Forbidden, 544 KB error page)

   **No successful HTTP 200 full-text snapshot exists in Wayback.**

3. Memento Time Travel API (`timetravel.mementoweb.org/api/json/2024/...`):
   HTTP 000 / no response.

4. Unpaywall API:
   `curl 'https://api.unpaywall.org/v2/10.1016/j.jclepro.2021.127043?email=...'`
   → `is_oa: False`, `oa_locations: []`. **No OA copy anywhere known to
   Unpaywall.**

**Reason:** Paywalled (Elsevier). No green-OA preprint registered. Wayback
captured only the redirect/error pages, never the article body.

**How to close:** institutional / library access via the DOI URL, or a
later Wayback snapshot if Elsevier ever changes their robots policy.

### 1B. Chabbi 2017 (Measurement 95:99–115)

**DOI:** `10.1016/j.measurement.2017.10.067`
**ScienceDirect PII:** S0263224117307029 (confirmed via Crossref).

**Round-2 attempts:**

1. Wayback CDX search:
   `curl 'https://web.archive.org/cdx/search/cdx?url=sciencedirect.com/science/article/pii/S0263224117307029&output=json&filter=statuscode:200&limit=20'`
   → Returns `[]` — **zero successful snapshots in Wayback history.**

2. Memento Time Travel API: HTTP 000.

3. Unpaywall API: `is_oa: False`, `oa_locations: []`.

**Reason:** Paywalled (Elsevier), and unlike Trifunović 2021, Wayback has
never successfully archived this article URL at all.

**How to close:** same as 1A.

### 1C. Alternative open POM-C primary sources searched

- EuropePMC (`polyacetal OR polyoxymethylene "specific cutting"`): 15 hits,
  none on-target (mostly biomedical mandibular-osteotomy "cutting guides").
- EuropePMC (`"POM-C" cutting force milling`): top hits return generic
  manufacturing reviews + the PMC9864973 paper (analysed below).
- **PMC9864973** — *"Determination of Processing Precision of Hole in
  Industrial Plastic Materials"* (Polymers 15(2):347, 2023, DOI
  10.3390/polym15020347). Includes POM-C as a tested material. **Fetched
  full PDF (10 pages, EuropePMC pdf=render endpoint).** Reading the paper:
  output responses are hole diameter / position precision, **not cutting
  force / SCE**. No Fc data extractable.
- **PMC9459756** — *"Experimental Investigation and Optimization of Turning
  Polymers Using RSM..."* (Polymers 14(17):3585, 2022). HDPE + PA6 turning;
  abstract mentions Fc as one of the responses, but the actual analyzed
  outputs in Tables 3 / Eqs (1)–(6) are Ra, MRR, and chip-thickness ratio λc
  — **no Fc values extractable from the paper text** (likely only in
  figures, which the EuropePMC text extract doesn't expose; the figure files
  are PDFs of plots without numeric tables). Also: this is HDPE/PA6, not
  POM-C.

**Disposition.** POM/Delrin Kc remains GAP. No primary fetched measurement
of POM specific cutting force or SCE obtained in Round-2.

---

## 2. PMMA / Acrylic — Korkmaz 2017 fetch unsuccessful; alternative fetched

### 2A. Korkmaz 2017 re-fetch (open access, but anti-bot 403)

**DOI:** `10.1016/j.promfg.2017.07.017`
**ScienceDirect PII:** S235197891730197X
**Status (Unpaywall):** `is_oa: True`, license CC-BY-NC-ND — **but** the
"best_oa_location" is the publisher landing page itself (no repository copy
exists).

**Round-2 attempts:**

1. DOI direct: `curl https://doi.org/10.1016/j.promfg.2017.07.017`
   → HTTP 200, body is the Elsevier redirect intermediate (not article text).

2. ScienceDirect direct (with `Mozilla/5.0` UA):
   `curl 'https://www.sciencedirect.com/science/article/pii/S2351978917301970'`
   → HTTP 403 (anti-bot challenge, 835 KB body).

3. ScienceDirect `/pdf` endpoint:
   `curl 'https://www.sciencedirect.com/science/article/pii/S235197891730197X/pdf'`
   → HTTP 403 (832 KB HTML challenge).

4. Elsevier text-mining REST endpoint:
   `curl 'https://api.elsevier.com/content/article/PII:S235197891730197X?httpAccept=text/plain'`
   → HTTP 400 (requires API key, which our environment doesn't have).

5. Wayback snapshot of the ScienceDirect URL: CDX search returns 3 HTTP 200
   snapshots in 2021–2022.
   `curl https://web.archive.org/web/20220203140457/https://www.sciencedirect.com/science/article/pii/S235197891730197X`
   → HTTP 200, 95 KB body. **Body contains only the abstract** (verbatim
   confirmed: "This paper presents an experimental investigation on
   micromachinability characteristics of Poly(methyl methacrylate)..."),
   **not the figures** with cutting-force magnitudes that would allow back-
   calculation. ScienceDirect's article HTML lazy-loads figure data via
   JavaScript that Wayback didn't capture.

6. CORE.ac.uk academic aggregator:
   `curl 'https://api.core.ac.uk/v3/search/outputs?q=doi:10.1016/j.promfg.2017.07.017'`
   → 1 hit, `fullText: ""`, `fulltextStatus: "disabled"`. Repository entry
   exists but no full-text body.

7. DuckDuckGo HTML search for `"Korkmaz" "PMMA" "single-crystal diamond"
   filetype:pdf` → returns ScienceDirect (paywalled), ResearchGate (403),
   and CORE.ac.uk entries. No usable open-access PDF mirror found.

**Reason:** Despite the paper being open-access per its CC-BY-NC-ND license,
Elsevier's anti-bot tier hard-blocks every direct fetch path, and no
repository / preprint server hosts a green-OA copy.

**How to close:** browser session with a residential IP (not CDN/cloud)
might pass the bot challenge; or institutional/library access; or wait for
a Wayback snapshot that captures the full lazy-loaded HTML (unlikely under
current ScienceDirect behaviour).

### 2B. Alternative PMMA primary force sources (Round-2 fetched ones — see kc_extra.md)

- **PMC12473824** (Liu et al. 2025, *Polymers* 17(18):2441) — fetched, has
  back-calculatable PMMA cutting force at 200 nm chip thickness. Recorded in
  `kc_extra.md` with size-effect caveat. **Not a milling-applicable Kc** —
  see caveat in `kc_extra.md`.
- **PMC12734954** (*"Wall Deformation and Minimum Thickness Analysis in
  Micro-Milled PMMA Microfluidic Devices"*, fetched) — focused on wall
  deformation, no Fc / SCE measurements.
- **PMC12829812** (*"Machinability and tribological optimization of origami-
  inspired Almond Shell-PMMA via RSM, ML, and TOPSIS"*, fetched) — measures
  cutting force on the **composite** (almond-shell-reinforced PMMA) per
  Table 2 DOE (10.5–15.8 N range across 27 trials at 6 mm end mill, feed
  0.05–0.15 mm/rev, DOC 0.2–0.6 mm), but does NOT report a baseline pure-
  PMMA cutting force. Quoted comparisons to "pure PMMA" cover hardness,
  tensile, wear and friction only — not Fc. No back-calculable pure-PMMA Kc.

---

## 3. Polycarbonate (PC) — Round-2 confirms no primary source exists

### Sources attempted in Round-2 (all yielded no on-target primary measurement)

1. **PMC search** (`polycarbonate "cutting force" milling`): 26 hits.
   Inspection of top 10 (by recency and apparent relevance): all are either
   different materials (vitrimers, PDMS, FRP composites), ultraprecision
   reviews without per-material Kc data, or 3D-printing / drilling-precision
   papers without force measurement. None measure PC cutting force.

2. **PMC search** (`"polycarbonate" "specific cutting"`): 21 hits. Same
   pattern — top result PMC12547080 is neuroimaging (mention of
   "polycarbonate" is a lab equipment material, not a workpiece); other hits
   are biomedical or unrelated.

3. **EuropePMC search** (`polycarbonate turning cutting force`): 15 hits,
   dominated by patient-specific surgical cutting guides — none on PC
   machining-force.

4. **EuropePMC search** (`"polycarbonate" "cutting force" milling OR
   turning`): same patient-guide noise + PMC12526530 (vitrimer, not PC).

5. **Manufacturer datasheets** (Covestro Makrolon, SABIC Lexan): publish
   mechanical (tensile, flexural, Izod, HDT) and general tooling
   recommendations (e.g. spindle speed ranges, recommended sharp tools) but
   **no Kc value or cutting-force coefficient**. This matches the Round-1
   finding.

6. **Google Scholar query** `polycarbonate cutting force specific kc
   milling`: top hits are general polymer-machining textbooks (e.g. Sheikh-
   Ahmad 2009 *Machining of Polymer Composites*, Springer) which discuss PC
   qualitatively but do not publish a Kc number. Sheikh-Ahmad's textbook is
   itself paywalled.

**Reason:** PC primary machining-force literature is genuinely sparse — the
material is well-characterised mechanically (tensile, impact, optical) but
not from a cutting-mechanics perspective. PC is typically machined for
optical / medical parts where surface roughness and burr are the focus, not
specific cutting force.

**Disposition.** PC's status remains: **NO PRIMARY SOURCE EXISTS** (the
architecturally correct "refuse the gate" state per Phase 1A.0). This is a
defensible, evidence-backed finding and itself the correct outcome.

---

## SUMMARY OF ROUND-2 GAPS

| Material | Round-1 status | Round-2 status | Reason |
|---|---|---|---|
| POM / Delrin | Paywalled | **Paywalled** (re-confirmed via Wayback CDX + Unpaywall) | Trifunović 2021 Wayback has only 302/403 snapshots; Chabbi 2017 has zero Wayback snapshots; Unpaywall confirms is_oa=False for both |
| PMMA / Acrylic | Korkmaz 2017 Fc unfetched | **Korkmaz 2017 still unfetched** (Wayback captured only abstract, not figures). New PMC12473824 nanoscale Fc fetched and back-calculated — recorded in kc_extra.md with explicit caveat that it's NOT a milling-applicable Kc | Elsevier anti-bot 403 across all fetch paths; no green-OA repository copy; nanoscale alt source is size-effect-inflated |
| Polycarbonate | No primary source | **No primary source** (re-confirmed via PMC + EuropePMC + manufacturer datasheets) | Genuinely sparse literature; PC studied for optics not cutting mechanics |
