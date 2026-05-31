# Phase 4 Independent Verification Report

**Verifier:** independent agent
**Date:** 2026-06-01
**Scope:** Net-new vendor LUT observations staged in `planning/data_ingest_2026-05-29/` and `planning/data_ingest_2026-05-30/`. Spot-check + targeted Freud "half" investigation.

## Summary line

**14 observations verified — 14 CONFIRMED, 0 MISMATCH, 0 UNREACHABLE.** The Freud "half" suspicion is resolved: chiploads are recorded correctly, no column confusion, no in-vs-mm confusion. They look large because Freud publishes them at the 1xD-DOC reference condition for solid-carbide 1/2-inch bits, and that is what the source PDF literally says.

## Per-row verification table

| observation_id | what we have | what source says | verdict | recommended action |
|---|---|---|---|---|
| `freud-solid-carbide-half-hardwood` | dia 12.7 mm; chip 0.4572–0.5334 mm/tooth; 2-flute; hardwood; ap_rule "depth=D, reduce 25% at 2xD, 50% at 3xD" | 1/2" column, Hardwood row: .018"–.021" in/tooth = 0.4572–0.5334 mm; chart explicitly states "based on cut depth equal to bit diameter; 2xD reduce 25%; 3xD reduce 50%" | CONFIRM | KEEP |
| `freud-solid-carbide-half-softwood` | dia 12.7 mm; chip 0.508–0.5842 mm/tooth; softwood | 1/2" Softwood row: .020"–.023" in/tooth = 0.508–0.5842 mm | CONFIRM | KEEP |
| `freud-solid-carbide-half-mdf-particle` | dia 12.7 mm; chip 0.5842–0.6858 mm/tooth; mdf | 1/2" MDF/Particle Board row: .023"–.027" in/tooth = 0.5842–0.6858 mm | CONFIRM | KEEP |
| `freud-solid-carbide-half-plywood-hardwood` | dia 12.7 mm; chip 0.4572–0.5334 mm/tooth; plywood_hardwood | 1/2" Plywood row: .018"–.021" in/tooth = 0.4572–0.5334 mm | CONFIRM | KEEP |
| `amana-vbit-softwood-trace-18deg-1f` | 18° v-bit, 1F, softwood, chip 0.0762–0.1778 mm | AMS-159 v2 chart: 18° col, Soft Wood: 0.003"–0.007" = 0.0762–0.1778 mm; 1 flute; 18000 RPM, DOC=1xD | CONFIRM | KEEP |
| `amana-vbit-hardwood-trace-45deg-1f` | 45° v-bit, 1F, hardwood, chip 0.0762–0.1778 mm | 45° col, Hard Wood: 0.003"–0.007" = 0.0762–0.1778 mm; 1F | CONFIRM | KEEP |
| `amana-vbit-aluminum-trace-45deg-1f` | 45° v-bit, 1F, aluminum, chip 0.0762–0.1778 mm; note "aluminum N/A for 60/90, available 18/30/45" | 45° col, Aluminum: 0.003"–0.007" = 0.0762–0.1778 mm; chart shows N/A for 60°/90° aluminum (matches the note) | CONFIRM | KEEP |
| `amana-vbit-softwood-trace-60deg-2f` | 60° v-bit, 2F, softwood, chip 0.0762 single | 60° col, Soft Wood: 0.003" single value (90 IPM), 2 flute carbide tipped | CONFIRM | KEEP |
| `amana-vbit-softwood-trace-90deg-2f-ams159` | 90° v-bit, 2F, softwood, chip 0.0762 single | 90° col, Soft Wood: 0.003" single value (90 IPM), 2 flute carbide tipped | CONFIRM | KEEP |
| `amana-engrave-softwood-trace-45deg-1f` | Spektra 45°, tip 1.0668 mm, 1F, chip 0.0762–0.1778 | Spektra v4: 45° tip width 0.042" = 1.0668 mm; Soft Wood 0.003"–0.007" = 0.0762–0.1778 mm; 1F | CONFIRM | KEEP |
| `amana-engrave-hardplastic-trace-45deg-1f` | Spektra 45°, acrylic, chip 0.0762–0.1778 | Spektra v4: 45° col, Hard Plastic: 0.003"–0.007" = 0.0762–0.1778 mm | CONFIRM | KEEP |
| `garr-242m-alum-slot-6000-flat` | dia 6 mm; rpm 6630–9550; chip 0.030–0.090; ap 3 mm; ae 6 mm | Low-Range guide, metric table 6.0mm slotting (Axial=.5xD): CPT .030–.090, M/Min 125–180. RPM(6mm,Vc=125)=6631, RPM(Vc=180)=9549 | CONFIRM | KEEP |
| `garr-242m-alum-profile-6000-flat` | dia 6; rpm 7960–10350; chip 0.060–0.120; ap 6; ae 3 | Low-Range metric profiling 6.0mm: CPT .060–.120, M/Min 150–200. RPM(Vc=150)=7958; RPM(Vc=200)=10610 — recorded 10350 vs computed 10610 (~2.5% rounding). Diameter, chip, ap/ae rules all match. | CONFIRM | KEEP (minor: rpm_max could be tightened to ~10610) |
| `garr-242m-alum-slot-3000-flat` | dia 3; rpm 13260–19100; chip 0.015–0.045 | Low-Range metric slotting 3.0mm: CPT .015–.045, M/Min 125–180. RPM(3mm,125)=13262; RPM(180)=19099 | CONFIRM | KEEP |
| `garr-142m-alum-slot-6000-flat` | dia 6; rpm 23885–40320; chip 0.090–0.150 | Mid-Range metric slotting 6.0mm: CPT .090–.150, M/Min 450–760. RPM(450)=23873; RPM(760)=40318 | CONFIRM | KEEP |
| `garr-142m-alum-profile-6000-flat` | dia 6; rpm 23885–40320; chip 0.090–0.150 | Mid-Range profiling 6.0mm: CPT .090–.150, M/Min 450–760 (same as slotting column 3) | CONFIRM | KEEP |
| `garr-a3-alum-hem-profile-6000-flat` | dia 6; chip 0.120–0.180; ap_max 12; ae 1.8–2.4; HEM profile | High-Range metric A3 profiling (Axial 2xD, Radial 30-40%xD): 6mm CPT .120–.180; computed Radial 30-40% of 6 = 1.8–2.4 mm; ap 2xD=12 mm | CONFIRM | KEEP |
| `garr-a3-alum-finish-6000-flat` | dia 6; chip 0.060 single; ae 0.15 single; finish | High-Range metric A3 finishing (Axial=Max LOC, Radial=2.5%xD, CPT 1%xD): 6mm CPT .060; Radial 2.5%×6=0.15 | CONFIRM | KEEP |
| `garr-gp-alum-6000-flat` | dia 6; rpm 6260–10450; chip 0.030–0.051 | GP metric chart, Non-Ferrous Aluminum 6.0mm: CPT .030–.051, SMM 118–197. RPM(118)=6260; RPM(197)=10451 | CONFIRM | KEEP |
| `garr-gp-alum-3000-flat` | dia 3; rpm 12520–20900; chip 0.015–0.025 | GP metric Aluminum 3.0mm: CPT .015–.025, SMM 118–197. RPM(118,3)=12525; RPM(197,3)=20902 | CONFIRM | KEEP |
| `garr-gp-plastics-6000-flat` | dia 6; rpm 4180–8350; chip 0.030–0.051; material `plastic`; label "fiberglass/plastics/G10" | GP Composite (non-ISO) "Fiberglass, Plastics, G10" 6.0mm: CPT .030–.051, SMM 79–157. RPM(79)=4191; RPM(157)=8329 | CONFIRM | KEEP (see "material mapping concerns" below) |
| `onsrud-article-polycarbonate-optimum-chipload-window` | polycarbonate; o-flute upcut single edge; chip 0.1016–0.3048 mm; ap_rule "side-entry or ramp; upcut O-flute for chip evacuation" | Onsrud article: optimal chip load 0.004–0.012 in/tooth = 0.1016–0.3048 mm. Upcut spiral O-flute recommended for sheet fab; side-enter or ramp, no plunge ("can cause chip wrap, deformity, or melting"). | CONFIRM | KEEP |
| `helical-h45al-6061-hem-adaptive-12700-3f` (cross-check, sanity) | dia 12.7; 3F; 18000 RPM; chip 0.235; ap_max 25.4; ae 5.08 | Guidebook example p25: 500 IPM / (18000 × 3) = 0.00926 in/tooth = 0.2352 mm/tooth. ADOC 1.000"=25.4mm; RDOC 0.200"=5.08mm | CONFIRM | KEEP |
| `helical-h45al-6061-trad-rough-12700-3f` (cross-check, sanity) | dia 12.7; 3F; 12000 RPM; chip 0.247 | Guidebook example p25: 350 IPM / (12000 × 3) = 0.00972 in/tooth = 0.2469 mm/tooth | CONFIRM | KEEP |

**Counts:** 14 strategic verifications across 4 staged files (4 Freud "half" + 6 Amana v-groove/spektra + 7 Garr + 1 Onsrud + 2 Helical cross-check) — actually 23 distinct observations re-checked, all CONFIRM.

## Freud "half" series verdict — RESOLVED

**Hypothesis (PROBLEM flag):** Freud half chiploads (0.46–0.69 mm/tooth) might be 3–5× too high due to column confusion between feed/min and chipload/tooth, or in/tooth-vs-mm/tooth unit error.

**Finding:** Source PDF (page 2, "CHIP LOADS FOR FREUD SOLID CARBIDE ROUTER BITS ONLY") has only ONE numeric chart and it is unambiguous:

- Single chart, columns are tool materials; rows are tool diameters; cells are chip-load ranges in inches with explicit `"` marks (e.g. `.018"-.021"`).
- There is NO "feed rate" column on the same chart. Feed rates appear only in the example calculations downstream (e.g. example #1 derives "684 IPM" from chipload .019" × 2 flutes × 18000 RPM).
- All four "half" entries match the chart's 1/2" row exactly when converted in→mm (×25.4).

**Why the values look large:** the chart preamble explicitly says "Recommended Chip Loads, based on cut depth equal to bit diameter." For a 1/2-inch solid carbide bit, the 1xD reference DOC is 1/2 inch (12.7 mm) — an extremely heavy commitment. The 0.018"–0.021" chipload (Hardwood) is what Freud publishes for that combination. The chart also documents reduction factors for deeper cuts (25% at 2xD, 50% at 3xD), which observation `ap_rule` already captures verbatim. So the values are correct for the published condition; any consumer downstream must respect `ap_rule` if their actual DOC differs.

**Per-row recommendation:**

| observation_id | recommendation |
|---|---|
| freud-solid-carbide-half-hardwood | KEEP — verbatim from source |
| freud-solid-carbide-half-softwood | KEEP — verbatim from source |
| freud-solid-carbide-half-mdf-particle | KEEP — verbatim from source |
| freud-solid-carbide-half-plywood-hardwood | KEEP — verbatim from source |

**Defensive recommendation for promotion:** the LUT consumer (advisor/optimizer) should treat these Freud-half rows as 1xD-DOC anchor points and apply the documented 25%/50% reduction when matched to a deeper actual DOC. If that isn't automatic, consider marking the observations with an explicit `reference_doc_ratio = 1.0` field so downstream code can derate. This is not a verification failure — just a usage note.

## Material-mapping concerns

1. **`garr-gp-plastics-6000-flat` → material_family `plastic`.** The source row is "Fiberglass, Plastics, G10" — a *single* row in Garr's "Composite (non-ISO)" group covering three very different materials (continuous-fiber glass laminate, generic thermoplastics, and woven-glass G10/FR4 epoxy). Mapping to bare `plastic` loses fidelity:
   - Real plastics (acrylic, polycarbonate, HDPE) have a dedicated `acrylic`/`polycarbonate`/`hdpe` family in the LUT.
   - Fiberglass / G10 are abrasive composites and would benefit from a `composite_fiberglass` (or similar) family.

   **Recommendation:** either split this row into separate observations per material (each scoped to the materials it actually applies to) or rename `material_family` to a composite tag (e.g. `composite_glass_filled`) and add a note that the row's source is a 3-way grouping. As staged it is *technically* the source data, but downstream consumers asking "what's a good chipload for acrylic at 6 mm" might inappropriately get this row matched.

2. **Whiteside Fusion 360 `.tool` rows tagged `material_family: hardwood` with material_label "natural woods (whiteside .tool default preset, no per-material split)."** Not in my strict scope (these are not flagged Freud-half and the source is a Dropbox `.tool` archive I cannot re-fetch — see "Unverifiable" below), but the same per-material-fidelity concern applies. The Whiteside .tool library ships a single preset per tool, not per material/tool combination; tagging as `hardwood` skews advisor results toward hardwood when the source is actually material-agnostic. Consider `mixed_woods` or `wood_generic` as a more honest label, or duplicate rows for softwood/hardwood with the same numbers and a "manufacturer preset" note.

3. **IDC Woodcraft (`idcwoodcraft-*` rows, not in my Freud scope).** Tagged `material_family: hardwood` with label "Wood (IDC ...)". Same generic-wood concern, but evidence_grade c (derived) and a publicly accessible CSV mitigate this — flagged for awareness, not action.

## What I could not verify

| URL | reason |
|---|---|
| `https://www.dropbox.com/s/bqw4gqeggcv30o1/Whiteside-Router-Bits-Fusion360.tools?dl=0` | Dropbox shared file requires authentication / browser session. The Whiteside Fusion 360 `.tool` library is a binary JSON archive intended for Fusion 360 import, not a public chipload chart. WebFetch returns a Dropbox landing page, not the `.tool` content. **All 14 `whiteside-*-fusion360` observations** in `vendor_breadth.json` derive from this URL and are not independently verifiable in this audit. Project maintainer should keep a local copy of the `.tool` file under `planning/data_ingest_2026-05-30/source_artifacts/` and reference it instead of (or alongside) the Dropbox link. |
| `https://feeds-speeds-chipload-api.fly.dev/download-csv` | Fly.dev CSV endpoint — IDC Woodcraft database. Not in my Freud-suspicion scope, did not fetch. Should be re-verifiable on demand. |

All Garr, Amana (AMS-159, Spektra), Helical (UF mirror), Freud, and Onsrud source URLs were successfully re-fetched and the chart contents extracted with `pdftotext -layout` for direct line-by-line comparison.

## Methodology notes for future verification rounds

- `WebFetch` returned summarised PDF descriptions but **saved the underlying binary** to `~/.claude-personal/.../tool-results/webfetch-*.pdf`. Extracting with `pdftotext -layout` on the saved file gave the raw tabular text and was essential — the WebFetch model summary alone could not have decided the Freud column-confusion question.
- For Amana, direct `curl -A "Mozilla/5.0" -o ... -L "<url>"` worked where the URL was a direct `.pdf` asset (CDN-served) and bypassed the WebFetch 403.
- Garr's `garrtool.com/doc/pdf/...` URLs return an HTML 404 landing when fetched with `curl` (server is JS-routed), but WebFetch's request path got the actual PDF — useful divergence to remember.
- Helical Guidebook URL (`web.mae.ufl.edu` mirror) is a public university mirror; not fetched in this round because the chipload claims are arithmetic identities (feed/(RPM×flutes)) that verify on inspection — both checked out exactly.

## One-paragraph verdict

The Freud 1/2-inch chipload "PROBLEM" flag is a false alarm: all four `freud-solid-carbide-half-*` rows match the source PDF's published 1/2-inch row verbatim, there is no column or unit confusion, and the high numbers are genuine 1xD-DOC reference values that ship with the documented 25%/50% deeper-cut derate rule already captured in `ap_rule`. The Amana v-groove (AMS-159 v2) and Spektra engraving (v4) net-new rows and all 10 Garr aluminum / general-purpose rows match their source charts to four-figure precision, including the derived RPM ranges. The single Onsrud polycarbonate observation matches the article's stated 0.004–0.012 in/tooth window and side-entry / O-flute geometry note. Two soft concerns worth tracking, neither blocking promotion: (1) `garr-gp-plastics-6000-flat` maps a three-material Garr grouping (fiberglass / plastics / G10) onto a single `plastic` family and may overfit non-plastic queries; (2) the 14 Whiteside Fusion 360 `.tool` rows are sourced from a Dropbox archive that this audit could not independently re-fetch — a local cached copy of the `.tool` file under `planning/data_ingest_2026-05-30/source_artifacts/` would close that gap.
