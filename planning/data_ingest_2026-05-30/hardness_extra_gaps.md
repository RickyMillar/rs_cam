# Hardness Data Ingest (Extension) — Gaps Log — 2026-05-30

Companion to `hardness_extra.md`. Records sources that were attempted but
returned no usable hardness value (or returned only ranges without a
citeable single-grade datum). Each row is a failed-but-attempted fetch —
useful for the next ingest pass.

## H.1 — Plastics gaps

| Target | Source attempted | Outcome |
|---|---|---|
| UHMW-PE — Shore D from MakeItFrom material-group page | https://www.makeitfrom.com/material-properties/Ultra-High-Molecular-Weight-Polyethylene-UHMW-PE | Page exists; mechanical-property block lists density, modulus, tensile, elongation, but **no Shore D / Rockwell row** at the material-group level. Filled instead from Mitsubishi TIVAR 1000 datasheet (see `hardness_extra.md`). |
| UHMW-PE — Shore D from A&C Plastics distributor datasheet | https://www.acplasticsinc.com/media/documents/PD_UHMW.pdf | Datasheet header has `Hardness, Durometer, Shore "D" Scale, 73° F   -   ASTM D2240` row but the **value cell is "-"** (left blank by distributor). Not citeable. |
| Rigid PVC — Shore D from MakeItFrom uPVC page | https://www.makeitfrom.com/material-properties/Unplasticized-Rigid-Polyvinyl-Chloride-uPVC-PVC-U | No Shore D / Rockwell row published on that page. Filled instead from Interstate AM (Shore D 74), with the ASTM-method caveat noted. |
| Nylon 6/6 — Shore D / Rockwell on MakeItFrom PA66 page | https://www.makeitfrom.com/material-properties/Polyamide-PA-Nylon-6-6-66-Nylon-101 | Property tables list elastic modulus, elongation, flexural / tensile properties, but **no hardness row** on the material-group summary. Filled instead from Nylatron GS (filled grade) + Zytel 101L (DuPont unfilled grade) — both genuine manufacturer datasheets. |
| PETG — Shore D on MakeItFrom PETG page | https://www.makeitfrom.com/material-properties/Glycol-Modified-Polyethylene-Terephthalate-PETG-PET-G | Lists only `Rockwell R Hardness: 120`. No Shore D. Filled with Plaskolite VIVAK Rockwell R 115 (ASTM D785) — single citeable grade. |
| PETG — Shore D from Vivak fabrication / data PDFs | https://www.usplastic.com/catalog/files/specsheets/PET-GVIVAK.pdf, https://eagle-plastics.com/wp-content/uploads/2019/10/VIVAK-PETG.pdf | Both VIVAK PDFs only report Rockwell R-Scale, not Shore D. |
| Original WebFetch attempts on `makeitfrom.com` slugs that turned out to be wrong | `/Ultra-High-Molecular-Weight-Polyethylene-UHMWPE`, `/Rigid-Polyvinyl-Chloride-RPVC-UPVC`, `/Polypropylene-PP`, `/Polyamide-66-Nylon-66-PA66`, `/Polyethylene-Terephthalate-Glycol-PETG`, `/PETG-Polyethylene-Terephthalate-Glycol`, `/Polyamide-66-Polyamide-66-PA66`, `/Polyamide-66-PA-66-Nylon-66`, `/Nylon-66-Polyamide-66-PA66-Cast`, `/Polyethylene-Terephthalate-Glycol-Modified-PETG` | 404. Resolved via WebSearch domain-filter to `makeitfrom.com`; correct slugs found and re-fetched. |

## H.2 — Aluminum gaps

| Target | Source attempted | Outcome |
|---|---|---|
| 1100-O Brinell from ASM Aerospace Specification Metals | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma1100o (and ma1100, ma1100h12, ma1100h14, ma1100h16, ma1100h18, ma1100h) | All return HTTP 500. ASM's bassnum catalog does not host the 1xxx pure-aluminum series. Filled from MakeItFrom (Brinell 23) with scale-only ASTM caveat. |
| 3003-H14 Brinell from ASM | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma3003h14 (and ma3003h12/h16/h18/o/h) | All return HTTP 500. Same gap as 1100. Filled from MakeItFrom (Brinell 42). |
| MatWeb direct datasheet for 1100-O / 3003-H14 (non-ASM URLs) | https://www.matweb.com/search/datasheet.aspx?MatGUID=... | 403 Forbidden — MatWeb gates non-ASM pages behind login/session. |
| 7050-T7651 Brinell from MakeItFrom | https://www.makeitfrom.com/material-properties/7050-T7651-Aluminum | Page exists but the published mechanical block omits Brinell HB. Filled from ASM (HB 147) + Kaiser mill data (HB 150). |

## General observations

- **MakeItFrom URL slug discovery is non-trivial.** The site uses
  trademark / IUPAC long-form slugs (e.g. `Polyamide-PA-Nylon-6-6-66-Nylon-101`,
  `Glycol-Modified-Polyethylene-Terephthalate-PETG-PET-G`,
  `Unplasticized-Rigid-Polyvinyl-Chloride-uPVC-PVC-U`). The reliable
  discovery path is `WebSearch allowed_domains=["makeitfrom.com"]` then
  `WebFetch` the canonical hit — direct slug guessing returned 404 on every
  first attempt for UHMW, PVC, PETG, PP-H, PA66.
- **ASM `asm.matweb.com` data is aerospace-biased.** The 2xxx, 5xxx (sheet
  alloys), 6xxx, and 7xxx series are well covered; the 1xxx (commercially
  pure) and 3xxx (Mn-alloyed) series are not in the ASM bassnum catalog
  and return HTTP 500. For 1100/3003 hardness we have to use either
  MakeItFrom (no explicit ASTM method) or fall back to ASM-handbook chapter
  data (not fetched in this beat).
- **TLS chain issue on `asm.matweb.com`** persists from the 2026-05-29
  ingest; `curl -sk` (insecure) is the workaround. Recorded here so a
  future beat doesn't waste cycles re-debugging.
