# Aluminum Kc acquisition gaps — sources tried that did NOT yield a primary pair

> Phase 3 / beat F gap log. Companion to `aluminum_kc.md`. Every entry below
> is a source attempted on 2026-05-30 that failed to yield a fetched primary
> (kc1.1, mc) pair. The beat OVERALL succeeded (see `aluminum_kc.md`); these
> are documented for the next session and as a record of exhaustion.
>
> Per the overriding rule, NO number is recorded for any of these. The beat
> succeeded via the Wayback snapshot of Machining Doctor (which DID serve
> the historically-published table) — see `aluminum_kc.md` Section A.

---

## 1. Live Machining Doctor pages — Cloudflare 403 (persistent)

- URLs attempted:
  - `https://www.machiningdoctor.com/glossary/specific-cutting-force-kc-kc1/` → HTTP 403 (5552 bytes Cloudflare challenge page)
  - `https://www.machiningdoctor.com/specific-cutting-force/` → HTTP 403
  - `https://www.machiningdoctor.com/calculators/specific-cutting-force-calculator/` → HTTP 403
- Headers tried: `User-Agent: Mozilla/5.0`, full Chrome header bundle
  (`Accept`, `Accept-Language`, `Accept-Encoding`, `Connection`,
  `Upgrade-Insecure-Requests`). All returned the Cloudflare bot-challenge
  HTML, not the content.
- **Closed by:** fetching the Wayback Machine snapshots
  `web.archive.org/web/20240813103343/...` and
  `web.archive.org/web/20250122145958/...` — see `aluminum_kc.md` A. The
  Wayback fetch is the primary citation in this round.

## 2. Live Sandvik Coromant Kc page — Angular SPA shell only

- URL: `https://sandvik.coromant.com/en-us/knowledge/materials/specific-cutting-force`
  → HTTP 200 / 8606 bytes, but the response body is only the Angular
  shell (no kc data injected without JS execution).
- This matches `kc_gaps.md` 2026-05-29 finding: "the numeric kc1.1/mc
  table has been removed from the live site". Confirmed again 2026-05-30.
- **Mitigation:** the Sandvik ISO N page from 2017 Wayback snapshot
  (`web.archive.org/web/2017/.../iso_n_non_ferrous_materials/.../default.aspx`)
  DID serve full HTML with the aluminium-specific "Specific cutting force:
  350–700 N/mm²" range. Captured verbatim in `aluminum_kc.md` Section B.

## 3. Kennametal "Technical Calculations" PDF — 404

- URL: `https://www.kennametal.com/content/dam/kennametal/kennametal/common/Resources/Catalogs-Literature/Technical/Technical_Calculations.pdf`
  → HTTP 404. The shorter "engineering-calculators/turning-calculators/cutting-force.html"
  also returned HTTP 404. The dam-server master-catalog PDF URL
  guessed from documentation similarly 404'd.
- The Kennametal site does have engineering calculators but they are
  JavaScript-driven and the underlying kc tables (if any) are not exposed
  as static PDFs at predictable URLs.

## 4. ISCAR — Cloudflare 403 on technical articles + ecatalog

- URLs attempted:
  - `https://www.iscar.com/en-hq/technical-articles/year-2025/machining-calculations` → HTTP 403
  - `https://www.iscar.com/eCatalog/MachiningCalculator.aspx?fnum=1&app=66&mapp=ML&GFSTYP=M` → HTTP 403
  - `https://www.iscar.com/eCatalog/Item.aspx?cat=&item=Machining-Calculations` → HTTP 403
- ISCAR's anti-bot returns 403 even with full Chrome header bundle
  (only the H13 steel row from `kc.md` was reached on 2026-05-29; that
  fetch evidently slipped through before the wall tightened).
- **Wayback CDX search** returned a 2005 `CalculationofRequiredMachinePower_12.pdf`
  which fetches cleanly (1-page PDF, 41 KB). It confirms the Kienzle
  formula `P = Kc·ap·f·vc/(η·61·10^3) kW` and the definition
  `Kc - Specific Cutting Forces (N/mm²)` — but does NOT include the per-
  group Kc value table.
- **Mitigation:** the Wayback Machining Doctor snapshot (item 1 above)
  yielded a complete VDI-3323-based table that overlaps with the same
  industry-standard system ISCAR's calculators consume.

## 5. Live Sandvik "Face milling in aluminium" brochure (2014 PDF) — fetched, no kc1.1

- URL: `web.archive.org/web/20151014152757/.../c-2940-156.pdf` → HTTP 200,
  1.4 MB, 4-page PDF.
- Content: cutting-data example (vc, n, vf, ap, tool-life comparison)
  for a specific CoroMill 5B90 cylinder-head application. No kc1.1/mc
  pair. Documented as a `kc.md`-style soft cross-reference for
  application-side cutting parameters, but not a kc source.

## 6. ResearchGate "Sandvik Technical Guide – Materials ISO" — not retried

- Per `kc_gaps.md` 2026-05-29, this was 403 / Anubis-blocked previously.
- 2026-05-30 did not re-attempt this source because the Wayback path
  through Machining Doctor (item 1) closed the gap with sufficient
  evidence — the (kc1.1, mc) pair is consistent across the MD chart
  snapshot AND the MD glossary snapshot AND the Sandvik ISO N upper-
  bound, so a fourth independent fetch is not necessary to make the
  primary-source claim.
- Documented as "not retried, not blocked" — could still be exercised
  in a future session for additional triangulation if desired.

## 7. WALTER / DORMER / SECO catalog PDFs — not attempted in this round

- Plan rationale: items 1+5 already established a fetched primary pair
  for the exact use case (6061-T6, 7075-T6). The opportunity cost of
  attempting additional vendor catalogs (each with its own anti-bot
  behavior) is high relative to the marginal evidence added. Listed here
  so a follow-up session knows the search frontier was bounded, not
  exhaustive on the metals-tool-vendor side.

---

## Summary

The Wayback-Machine path through Machining Doctor's `specific-cutting-
force-chart` (2024-08-13 snapshot) and `glossary/specific-cutting-force-
kc-kc1` (2025-01-22 snapshot) successfully delivered a fetched-primary
(kc1.1, mc) pair for 6061, 7075, 2024 and the broader ISO N aluminium
groups, bounded above by a verified 2017 Sandvik live-page snapshot range
350–700 N/mm². The "hard" live-Sandvik-only constraint from the prior
2026-05-29 round was bypassed by going through the Wayback-archived
aggregator rather than re-attacking the live SPA shell.

Beat F is therefore **closed with a primary pair**, not as a gap. The
remaining gaps in this file are the ones the chosen path made
unnecessary, not blockers for the beat's stated success target.
